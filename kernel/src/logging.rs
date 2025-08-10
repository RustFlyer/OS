use core::{
    fmt,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use driver::print;
use logger::LogInterface;
use mutex::SpinNoIrqLock;

static mut LOGOUT: AtomicBool = AtomicBool::new(false);
static LOG_LOCK: SpinNoIrqLock<()> = SpinNoIrqLock::new(());

static FLITER_LIST: [&[&str]; 3] = [&["objects/29"], &["/fd/3"], &["/fd/3"]];
static mut FILTER_ID: AtomicUsize = AtomicUsize::new(0);

pub fn print_in_color(args: fmt::Arguments, color_code: u8) {
    print!("\u{1B}[{}m{}\u{1B}[0m", color_code, args);
}

struct LogInterfaceImpl;

#[allow(static_mut_refs)]
#[crate_interface::impl_interface]
impl LogInterface for LogInterfaceImpl {
    fn print_log(record: &log::Record) {
        let _guard = LOG_LOCK.lock();

        if !can_log() {
            return;
        }

        if can_filter() {
            let id = unsafe { FILTER_ID.load(Ordering::Relaxed) };
            let s = format!("{}", record.args());
            if FLITER_LIST[id - 1]
                .iter()
                .filter(|x| s.contains(*x))
                .last()
                .is_none()
            {
                return;
            }
        }

        print_in_color(
            format_args!(
                "[{:>5}][{}:{}] {}\n",
                record.level(),
                record.file().unwrap(),
                record.line().unwrap(),
                record.args()
            ),
            logger::level2color(record.level()),
        );
    }
}

#[allow(static_mut_refs)]
pub fn can_log() -> bool {
    unsafe { LOGOUT.load(Ordering::Relaxed) }
}

#[allow(static_mut_refs)]
pub fn enable_log() {
    unsafe { LOGOUT.store(true, Ordering::Relaxed) };
    log::debug!("Log Enable");
}

#[allow(static_mut_refs)]
pub fn disable_log() {
    unsafe { LOGOUT.store(false, Ordering::Relaxed) }
}

#[allow(static_mut_refs)]
pub fn can_filter() -> bool {
    unsafe { FILTER_ID.load(Ordering::Relaxed) > 0 }
}

/// Enable Filter(1,2,3)
#[allow(static_mut_refs)]
pub fn enable_filter(id: usize) {
    assert_ne!(id, 0, "You should not set filter as zero");
    assert!(id <= 3, "You should not set filter level higher than 3");
    log::debug!("Filter Enable");
    unsafe { FILTER_ID.store(id, Ordering::Relaxed) };
}

#[allow(static_mut_refs)]
pub fn disable_filter() {
    unsafe { FILTER_ID.store(0, Ordering::Relaxed) }
}
