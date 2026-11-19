#pragma once

namespace remidy {
// Host for an instance that no UI or script observes. Plugin rescans neither
// rebuild the parameter list nor read every value back.
class HeadlessCLAPHost final : public RemidyCLAPHost {
protected:
    void paramsRescan(clap_param_rescan_flags) noexcept override {}
};
}
