#pragma once
#include "evolution_dll.hpp"
#include "../patches/uapmd/evolution-graphs/graphs.hpp"
#include "patch_into.hpp"
#include <fstream>

// Unpublished UAPMD graphs supply the plugin ABI only. Rust owns clocks,
// readiness, sample envelopes and the handoff to the control thread.
class Evolution final : public uapmd::AudioProcessingEventHandler, public repl_evolution_graphs::Source {
    uapmd::SequencerEngine& engine;
    void* state;
    std::array<int32_t,2> tracks;
    std::array<std::unique_ptr<uapmd::PreparedSequencerTrack>,2> voices;
    std::array<int32_t,2> instances{-1,-1};
    std::array<std::string,2> errors;
    std::array<bool,2> loading{};
    std::array<bool,2> enabled{};
public:
    Evolution(uapmd::SequencerEngine& e, std::array<int32_t,2> t, double hold, double fade)
        : engine(e), state(repl_evolution::api->create(hold,fade)), tracks(t) {
        if (!state) throw std::runtime_error("Evolution needs at least five seconds hold and positive fade");
        engine.addAudioProcessingEventHandler(*this);
        engine.resetTrackProcessingState(tracks[0],false,[&] {
            repl_evolution_graphs::active.store(this,std::memory_order_release);
        });
    }
    ~Evolution() {
        engine.resetTrackProcessingState(tracks[0],false,[&] {
            repl_evolution_graphs::active.store(nullptr,std::memory_order_release);
        });
        engine.removeAudioProcessingEventHandler(*this);
        voices = {};
        repl_evolution::api->destroy(state);
    }
    bool busy() const { return loading[0] || loading[1]; }
    bool supportsParallelTrackProcessing() const noexcept override {
        for (uint32_t slot=0;slot<2;++slot) {
            const auto status=repl_evolution::api->status(state,slot);
            if ((status==2 || status==3) && !voices[slot]->track().graph().supportsParallelTrackProcessing()) return false;
        }
        return true;
    }
    uapmd_graph::AudioPluginGraph* graph(int32_t track) noexcept override {
        for (uint32_t slot=0;slot<2;++slot)
            if (track==tracks[slot] && enabled[slot]) return &voices[slot]->track().graph();
        return nullptr;
    }
    bool pending() const { return repl_evolution::api->pending(state); }
    void set(const choc::value::ValueView& args) {
        const auto plan = sequence_plan(args,engine);
        const auto selected=args["tracks"];
        if (plan.track!=tracks[0] || !selected.isArray() || selected.size()!=2 ||
            selected[0].get<double>()!=tracks[0] || selected[1].get<double>()!=tracks[1])
            throw std::runtime_error("Sequence must address the two evolution tracks");
        if (repl_evolution::api->publish(state,&plan) != 0)
            throw std::runtime_error("Evolution sequence queue is full or invalid");
    }
    choc::value::Value status() const {
        auto result=choc::value::createEmptyArray();
        for (uint32_t s=0;s<2;++s)
            result.addArrayElement(choc::value::createObject("", "state",static_cast<int32_t>(repl_evolution::api->status(state,s)),
                "gain",repl_evolution::api->gain(state,s),
                "activity",repl_evolution::api->spinning(state),
                "instanceId",instances[s],"loading",loading[s],"error",errors[s]));
        return result;
    }
    void load(uint32_t slot, std::string format, std::string plugin, const std::string& file) {
        if (slot>=2 || !repl_evolution::api->claim(state,slot))
            throw std::runtime_error("Evolution slot is not silent and available");
        loading[slot]=true;
        errors[slot].clear();
        // Retire before create: never temporarily retain a third instance.
        voices[slot].reset();
        instances[slot]=-1;
        try {
            std::ifstream input(std::filesystem::u8path(file),std::ios::binary);
            if (!input) throw std::runtime_error("Cannot read evolution state");
            std::vector<uint8_t> bytes((std::istreambuf_iterator<char>(input)),{});
            voices[slot]=engine.prepareTrack();
            if (!voices[slot]) throw std::runtime_error("Cannot prepare evolution graph");
            engine.addPluginToPreparedTrack(*voices[slot],format,plugin,
                [this,slot,bytes=std::move(bytes)](int32_t id,std::string error) mutable {
                    if (id<0) { complete(slot,std::move(error)); return; }
                    instances[slot]=id;
                    // Patches go into instances on the main thread only.
                    auto apply=[this,slot,bytes=std::move(bytes)]() mutable {
                        auto* instance=voices[slot] ? voices[slot]->pluginInstance(instances[slot]) : nullptr;
                        if (!instance) { complete(slot,"The evolution instance was destroyed"); return; }
                        instance->bypassed(true);
                        instance->stopProcessing();
                        auto patched=repl_patch_into::into(instance,bytes,true);
                        if (!patched.error.empty() || patched.state.empty()) { loaded(slot,std::move(patched.error)); return; }
                        instance->loadState(std::move(patched.state),uapmd_plugin_hosting::StateContextType::Project,false,nullptr,
                            [this,slot](std::string error,void*) {
                                remidy::EventLoop::enqueueTaskOnMainThread([this,slot,error=std::move(error)]() mutable {
                                    loaded(slot,std::move(error));
                                });
                            });
                    };
                    if (remidy::EventLoop::runningOnMainThread()) apply();
                    else remidy::EventLoop::enqueueTaskOnMainThread(std::move(apply));
                });
        } catch (const std::exception& error) { complete(slot,error.what()); }
    }
    // Main thread: restarts the slot's instance after its state was loaded.
    void loaded(uint32_t slot,std::string error) {
        if (error.empty()) {
            auto* instance=voices[slot]->pluginInstance(instances[slot]);
            if (instance->startProcessing()!=0) error="Cannot start evolution plugin";
            else instance->bypassed(false);
        }
        complete(slot,std::move(error));
    }
    void complete(uint32_t slot,std::string error) {
        errors[slot]=std::move(error);
        if (!errors[slot].empty()) { voices[slot].reset(); instances[slot]=-1; }
        loading[slot]=false;
        repl_evolution::api->ready(state,slot,errors[slot].empty());
    }
    void beforeTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        const auto slot=event.track_index==tracks[0]?0:event.track_index==tracks[1]?1:-1;
        if (slot<0) return;
        enabled[slot]=false;
        if (event.frame_count<=0 || event.frame_count>8192 || !engine.isPlaybackActive()) return;
        auto& context=event.context;
        const auto frames=static_cast<uint32_t>(event.frame_count);
        const auto rate=context.masterContext().sampleRate();
        if (!std::isfinite(rate) || rate<=0) return;
        if (slot==0) {
            repl_evolution::api->pulse(state);
            repl_evolution::api->begin(state,frames,rate);
        }
        context.eventIn().position(0);
        struct Output { uapmd::AudioProcessContext* context; double rate; uint32_t ticks{}; } output{&context,static_cast<double>(rate)};
        enabled[slot]=repl_evolution::api->render(state,slot,frames,rate,
            [](void* user,uint32_t offset,uint8_t pitch,bool on,uint16_t velocity) {
                auto& out=*static_cast<Output*>(user);
                auto& input=out.context->eventIn();
                const auto ticks=static_cast<uint32_t>(std::llround(offset*31250.0/out.rate));
                const uint32_t words[]{0x00200000u|(ticks-out.ticks),
                    (on?0x40900000u:0x40800000u)|(static_cast<uint32_t>(pitch)<<8),static_cast<uint32_t>(velocity)<<16};
                if (input.position()+sizeof(words)<=input.maxMessagesInBytes()) {
                    std::memcpy(static_cast<uint8_t*>(input.getMessages())+input.position(),words,sizeof(words));
                    input.position(input.position()+sizeof(words)); out.ticks=ticks;
                }
            },&output);
    }
    void afterTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        const auto slot=event.track_index==tracks[0]?0:event.track_index==tracks[1]?1:-1;
        if (slot<0 || event.frame_count<=0 || event.frame_count>8192 || !engine.isPlaybackActive()) return;
        auto& context=event.context;
        const auto frames=static_cast<uint32_t>(event.frame_count);
        if (enabled[slot]) {
            for (uint32_t bus=0;bus<context.audioOutBusCount();++bus)
                for (uint32_t channel=0;channel<context.outputChannelCount(bus);++channel)
                    if (context.masterContext().audioDataType()==remidy::AudioContentType::Float64)
                        repl_evolution::api->mix64(state,slot,context.getDoubleOutBuffer(bus,channel),frames);
                    else repl_evolution::api->mix(state,slot,context.getFloatOutBuffer(bus,channel),frames);
        } else context.clearAudioOutputs();
        repl_evolution::api->finish(state,slot);
    }
};
