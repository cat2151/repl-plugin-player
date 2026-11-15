#include "segments.hpp"
#include <iostream>
#include <stdexcept>
#include <vector>

static void check(int64_t position, int32_t frames, bool looping, int64_t start, int64_t end,
                  const std::vector<int64_t>& expected) {
    std::vector<int64_t> output(frames, -999);
    repl_frozen_loop::read(position, frames, looping, start, end,
        [&](int64_t source, int32_t offset, int32_t count) {
            if (count <= 0 || offset < 0 || offset + count > frames)
                throw std::runtime_error("invalid output segment");
            for (int32_t i = 0; i < count; ++i) output[offset + i] = source + i;
        });
    if (output != expected) throw std::runtime_error("wrong cache sample positions");
}

int main() {
    try {
        check(0, 6, true, 0, 4, {0,1,2,3,0,1});
        check(4, 6, true, 0, 4, {0,1,2,3,0,1});
        check(10, 6, true, 2, 5, {4,2,3,4,2,3});
        check(0, 8, true, 2, 5, {0,1,2,3,4,2,3,4});
        check(3, 6, true, 2, 3, {2,2,2,2,2,2});
        check(3, 5, false, 2, 4, {3,4,5,6,7});
        check(3, 5, true, 4, 4, {3,4,5,6,7});
        check(3, 5, true, 5, 4, {3,4,5,6,7});
        check(-2, 5, true, 0, 2, {-2,-1,0,1,0});
        check(0, 0, true, 0, 4, {});
        if (repl_frozen_loop::wrap(100, true, 2, 5) != 4)
            throw std::runtime_error("loader position did not wrap");
        std::cout << "PASS: frozen cache loop segments and loader position\n";
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
