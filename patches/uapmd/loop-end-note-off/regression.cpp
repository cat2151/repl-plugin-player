#include <uapmd-data/uapmd-data.hpp>
#include "boundary.hpp"
#include <iostream>
#include <stdexcept>

static void check(bool condition, const char* message) {
    if (!condition) throw std::runtime_error(message);
}

int main() {
    try {
        // 48 kHz, 120 BPM, 480 PPQN: one tick = 50 samples.
        // Put an ordinary Note On BEFORE the release at the same endpoint.
        // A release after the range and a program change must also stay out.
        const std::vector<uint32_t> words{
            0x20903c64, 0x20904064, 0x20803c00, 0x20c00100, 0x20804000};
        const std::vector<uint64_t> ticks{0, 10, 10, 10, 11};
        uapmd::MidiClipSourceNode node(0, words, ticks, 480, 120, 48000, {}, {});
        auto messages = [](remidy::EventSequence& sequence) {
            std::vector<uint32_t> result;
            const auto* data = static_cast<const uint32_t*>(sequence.getMessages());
            for (size_t i = 0; i < sequence.position() / 4; ++i)
                if (data[i] >> 28) result.push_back(data[i]);
            return result;
        };
        for (bool boundary : {false, true}) {
            remidy::EventSequence sequence(4096);
            node.seek(0);
            node.setPlaying(true);
            repl_loop_end::Scope scope(boundary);
            node.processEvents(sequence, 500, 48000, 120, 17);
            const auto actual = messages(sequence);
            check(actual == (boundary ? std::vector<uint32_t>{words[0], words[2]} :
                std::vector<uint32_t>{words[0]}), "Endpoint admitted a Note On/program/future release or lost Note Off");
            if (boundary) {
                const auto* data = static_cast<const uint32_t*>(sequence.getMessages());
                uint32_t jr = 0;
                for (size_t i = 0; i < sequence.position() / 4; ++i) {
                    if (data[i] >> 28 == 0) jr += data[i] & 65535;
                    if (data[i] == words[2])
                        check(jr == (516 * 31250 + 24000) / 48000, "Release offset escaped outgoing block");
                }
            }
        }
        check(!repl_loop_end::includeEndNoteOff, "Boundary scope leaked");
        for (const auto& release : std::vector<std::vector<uint32_t>>{
                {0x20903c00}, {0x40803c00, 0x40000000}, {0x40903c00, 0x00001234}}) {
            std::vector<uint64_t> times(release.size(), 10);
            uapmd::MidiClipSourceNode source(0, release, times, 480, 120, 48000, {}, {});
            remidy::EventSequence sequence(4096);
            source.seek(0);
            source.setPlaying(true);
            repl_loop_end::Scope scope(true);
            source.processEvents(sequence, 500, 48000, 120);
            check(sequence.position() == (release.size() + 1) * 4, "MIDI1/2 release or velocity-zero release lost");
            // The same endpoint cannot be emitted twice by a continued source call.
            sequence.position(0);
            source.processEvents(sequence, 500, 48000, 120);
            check(sequence.position() == 0, "Boundary release duplicated");
        }
        check(!repl_loop_end::isNoteOff(0x40903c00, 0x00010000), "MIDI2 positive velocity classified as release");
        std::cout << "PASS loop endpoint Note Off filtering and offsets\n";
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
