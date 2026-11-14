#include "routing.hpp"
#include <stdexcept>
#include <vector>

namespace {
void check(bool value, const char* message) {
    if (!value) throw std::runtime_error(message);
}
void selection(uint8_t group, const std::vector<uint32_t>& expected) {
    // Two groups start together, then stop 0.1 seconds later. The second
    // timestamp also follows events belonging only to the other group.
    const uint32_t words[]{0x20903c64, 0x21904064, 0x00200c35,
                           0x20803c00, 0x21804000, 0x00200064,
                           0x4f904300, 0xc8000000};
    remidy::MasterContext master;
    remidy::AudioProcessContext input(master, 4096), output(master, 4096);
    std::memcpy(input.eventIn().getMessages(), words, sizeof(words));
    input.eventIn().position(sizeof(words));
    check(repl_dag::appendEventsForGroup(output.eventIn(), input.eventIn(), group), "event selection failed");
    check(output.eventIn().position() == expected.size() * sizeof(uint32_t), "selected event length changed");
    check(std::memcmp(output.eventIn().getMessages(), expected.data(), expected.size() * sizeof(uint32_t)) == 0,
          "group selection must preserve timing, order and complete messages");
    check(std::memcmp(input.eventIn().getMessages(), words, sizeof(words)) == 0, "selection mutated source");
    repl_dag::normalizeInstanceGroup(output.eventIn(), group);
    const auto* selected = static_cast<const uint32_t*>(output.eventIn().getMessages());
    for (size_t i = 0; i < expected.size(); ++i)
        if (expected[i] >> 28 == 0)
            check(selected[i] == expected[i], "port normalization changed timestamp");
}
}

void checkEventTiming() {
    selection(0, {0x20903c64, 0x00200c35, 0x20803c00, 0x00200064});
    selection(1, {0x21904064, 0x00200c35, 0x21804000, 0x00200064});
    selection(15, {0x00200c35, 0x00200064, 0x4f904300, 0xc8000000});
    selection(0xff, {0x20903c64, 0x21904064, 0x00200c35, 0x20803c00,
                     0x21804000, 0x00200064, 0x4f904300, 0xc8000000});
}
