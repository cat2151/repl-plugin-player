#include "metadata.hpp"
#include <stdexcept>

void require(bool condition) { if (!condition) throw std::runtime_error("metadata regression"); }
int main() {
    int owner{};
    using namespace repl_parameters;
    uint64_t first;
    {
        Capture capture(2);
        require(!observed);
        report(&owner, 1, 99, 0); require(!observed);
        report(&owner, 2, 123456, 8);
        require(observed && observed->stable_id == 123456 && (observed->flags & 8));
        first = observed->epoch;
    }
    require(requested == -1 && !observed);
    invalidate(&owner);
    {
        Capture capture(2);
        report(&owner, 2, 123456, 0);
        require(observed->epoch != first && observed->flags == 0);
        first = observed->epoch;
    }
    destroy(&owner);
    {
        Capture capture(2);
        report(&owner, 2, 123456, 1);
        require(observed->epoch != first && observed->flags == 1);
    }
    destroy(&owner);
    require(epochs.empty());
}
