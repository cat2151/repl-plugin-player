#pragma once
#include "../../../native/loop_playback.hpp"

namespace repl_loop {
// Deterministic render-ahead/latency cases use the production snapshot selector
// and MIDI emitter. Real plugin delivery and master signal are checked by E2E.
struct Regression {
    static void require(bool condition, const char* reason) {
        if (!condition) throw std::runtime_error(reason);
    }
    static std::unique_ptr<Plan> plan(uint64_t revision, uint32_t a, uint32_t b, int64_t length = 1000) {
        auto result = std::make_unique<Plan>();
        result->revision = revision;
        result->length = length;
        for (auto [name, note] : {std::pair{"a", a}, std::pair{"b", b}}) {
            Part part;
            part.track = name;
            // A note extending through the boundary; no pre-boundary Off.
            part.events.push_back({0, {0x20900064u | (note << 8), 0}, 1});
            result->parts.push_back(std::move(part));
        }
        return result;
    }
    static std::vector<std::array<uint32_t, 3>> read(remidy::EventSequence& sequence) {
        std::vector<std::array<uint32_t, 3>> result;
        const auto* words = static_cast<const uint32_t*>(sequence.getMessages());
        uint32_t offset{};
        for (size_t i = 0; i < sequence.position() / 4; ++i) {
            if ((words[i] >> 28) == 0) offset = words[i] & 65535;
            else result.push_back({offset, (words[i] >> 8) & 127, (words[i] >> 20) & 15});
        }
        sequence.position(0);
        return result;
    }
    static void run() {
        display();
        dynamic();
        Playback playback;
        playback.period.store(1000);
        playback.plans.publish(plan(1, 60, 67));
        uapmd::TimelineTrack a("a", 2, 48000, 256), b("b", 2, 48000, 256);
        remidy::EventSequence output(8192);
        playback.advance(-512, 128, 256);
        playback.segment(a, 0, output, -512, 0, 256, 0, true);
        require(read(output).empty(), "Negative preroll repeated the first note");
        playback.segment(a, 0, output, -128, 0, 256, 0, true);
        require(read(output) == std::vector<std::array<uint32_t, 3>>{{128, 60, 9}}, "Preroll crossing lost first note/offset");
        playback.segment(b, 1, output, 0, 0, 128, 0, true);
        read(output);
        playback.plans.publish(plan(2, 60, 69));
        playback.advance(100, 800, 256);
        require(playback.boundary == 1000, "Prepared update missed next feasible common boundary");
        playback.segment(b, 1, output, 800, 800, 256, 0, true);
        require(read(output) == std::vector<std::array<uint32_t, 3>>{{200, 67, 8}, {200, 69, 9}}, "Fast track mixed old/new or left a long note");
        playback.plans.publish(plan(3, 72, 74));
        playback.advance(356, 1056, 256);
        require(playback.guards[playback.pending]->revision == 2, "Update replaced a partially rendered multi-track transition");
        playback.segment(a, 0, output, 900, 900, 256, 0, true);
        require(read(output) == std::vector<std::array<uint32_t, 3>>{{100, 60, 8}, {100, 60, 9}}, "Slow track boundary/same-note retrigger disagreed");
        playback.advance(1156, 1856, 256);
        require(playback.guards[playback.active]->revision == 2 && playback.boundary == 2000,
                "Deferred latest update did not use first feasible render boundary");
        playback.playbackStopped();
        require(playback.pending == -1 && playback.plans.currentOnPublisherThread()->revision == 2, "Stop retained a stale reservation");
        playback.advance(0, 0, 256);
        playback.plans.publish(plan(4, 75, 76));
        playback.advance(990, 1490, 256);
        require(playback.boundary == 2000, "Near-boundary request modified an already rendered boundary");
        playback.plans.publish(plan(5, 77, 78));
        playback.advance(1000, 1500, 256);
        require(playback.guards[playback.pending]->revision == 5 && playback.boundary == 2000, "Latest unstarted update did not supersede its predecessor");
        require(playback.dropped.load() == 0, "Deterministic MIDI delivery overflow");
    }
    static void display() {
        Playback playback;
        playback.period.store(1000);
        playback.plans.publish(plan(1, 60, 67));
        playback.advance(0, 0, 256);
        playback.captureView(0);
        require(playback.view(true)["revision"].get<int64_t>() == 1, "Display omitted the first loop plan");
        playback.plans.publish(plan(2, 62, 69, 1500));
        playback.advance(100, 800, 256);
        playback.captureView(100);
        require(playback.view(true)["revision"].get<int64_t>() == 1, "Display showed an unapplied loop update");
        playback.advance(1100, 1800, 256);
        playback.captureView(900); // Audible transport is behind the render head.
        auto old = playback.view(true);
        require(old["revision"].get<int64_t>() == 1 && old["lengthSamples"].get<int64_t>() == 1000,
                "Render-ahead discarded the audible display plan");
        playback.plans.publish(plan(3, 65, 72, 750)); // Reclamation must retain the displayed plan.
        playback.captureView(1050);
        auto next = playback.view(true);
        require(next["revision"].get<int64_t>() == 2 && next["positionSamples"].get<int64_t>() == 50,
                "Display plan/phase did not change together at the audible boundary");
        require(next["parts"][0]["events"][0]["words"][0].get<int64_t>() == (0x20900064u | (62u << 8)),
                "Display serialized a different note from the playing plan");
        playback.playbackStopped();
        require(playback.view(false)["revision"].get<int64_t>() == 2,
                "Stopped display retained a canceled pending plan");
        playback.playbackStopped(); // App Stop and audio-engine disable both stop transport.
        require(playback.view(false)["revision"].get<int64_t>() == 2,
                "Repeated Stop erased the adopted loop plan");
    }
    static void dynamic() {
        Playback playback;
        playback.period.store(1000);
        playback.plans.publish(plan(1, 60, 60));
        uapmd::TimelineTrack track("a", 2, 48000, 256);
        remidy::EventSequence out(8192);
        playback.advance(0, 0, 256);
        playback.segment(track, 0, out, 0, 0, 256, 0, true);
        require(read(out) == std::vector<std::array<uint32_t, 3>>{{0,60,9}}, "Initial phase lost");
        auto longer = plan(2, 62, 62, 1500);
        longer->parts[0].events.push_back({1200, {0x20904064u,0},1});
        playback.plans.publish(std::move(longer));
        playback.advance(100, 800, 256);
        require(playback.boundary == 1000, "Extension missed old cycle boundary");
        playback.segment(track, 0, out, 800, 800, 256, 0, true);
        require(read(out) == std::vector<std::array<uint32_t,3>>{{200,60,8},{200,62,9}}, "Extension phase/off-on mismatch");
        require(playback.position(1000) == 0 && playback.position(1200) == 200 && playback.span(990) == 10,
                "New period incorrectly used raw modulo or skipped transition split");
        playback.plans.publish(plan(3, 65, 65, 750));
        playback.advance(356, 1056, 256);
        require(playback.guards[playback.pending]->revision == 2, "Partially rendered dynamic update was replaced");
        playback.advance(1100, 1800, 256);
        require(playback.boundary == 2500, "Shortening was not relative to new origin");
        require(playback.position(900) == 900 && playback.position(2499) == 1499 && playback.position(2500) == 0,
                "Transport disagreed across old/new origins");
        playback.segment(track, 0, out, 2200, 0, 500, 0, true);
        require(read(out) == std::vector<std::array<uint32_t,3>>{{0,64,9},{300,62,8},{300,64,8},{300,65,9}},
                "Extension tail lost, or shortening left held notes");
        playback.advance(2700, 2700, 256);
        for (int64_t i = 1; i <= 20; ++i) {
            const auto raw = 2500 + i * 750;
            require(playback.position(raw) == 0, "Phase drift proportional to cycle count");
            playback.segment(track, 0, out, raw - 3, 0, 10, 0, true);
            require(read(out) == std::vector<std::array<uint32_t,3>>{{3,65,8},{3,65,9}}, "Rounded cycle retrigger drift");
        }
        const auto rounded = Playback::sampleLength(1.234567, 120, 48000);
        require(rounded == 29630, "Fractional period rounding changed");
        playback.playbackStopped();
        playback.plans.publish(plan(4, 67, 67, rounded));
        playback.advance(0, 0, 256);
        for (int64_t i = 0; i < 20; ++i)
            require(playback.position(i * rounded) == 0, "Fractional repetition accumulated error");
        require(playback.dropped.load() == 0, "Dynamic delivery overflow");
    }
};
}
