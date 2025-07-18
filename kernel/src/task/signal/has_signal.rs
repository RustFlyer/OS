use net::tcp::HasSignalIf;
use osfs::pselect::PSFHasSignalIf;

use crate::processor::current_task;

use super::sig_info::SigSet;

struct HasSignalIfImpl;
#[crate_interface::impl_interface]
impl HasSignalIf for HasSignalIfImpl {
    fn has_signal() -> bool {
        let task = current_task();
        let mask = *task.sig_mask_mut();
        task.with_mut_sig_manager(|manager| manager.has_expect_signals(!mask))
    }
}

struct PSFHasSignalIfImpl;
#[crate_interface::impl_interface]
impl PSFHasSignalIf for PSFHasSignalIfImpl {
    fn has_signal() -> bool {
        let task = current_task();
        let mask = *task.sig_mask_mut() & !SigSet::SIGKILL;
        task.with_mut_sig_manager(|manager| manager.has_expect_signals(!mask))
    }

    fn has_expected_signal(sigset: SigSet) -> bool {
        let task = current_task();
        task.with_mut_sig_manager(|manager| manager.has_expect_signals(sigset))
    }
}
