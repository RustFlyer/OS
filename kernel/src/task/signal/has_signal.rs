use net::tcp::HasSignalIf;

use crate::processor::current_task;

struct HasSignalIfImpl;
#[crate_interface::impl_interface]
impl HasSignalIf for HasSignalIfImpl {
    fn has_signal() -> bool {
        let task = current_task();
        let mask = *task.sig_mask_mut();
        task.with_mut_sig_manager(|manager| manager.has_expect_signals(!mask))
    }
}
