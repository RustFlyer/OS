use core::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use driver::print;
use logger::LogInterface;
use mutex::SpinNoIrqLock;

static mut LOGOUT: AtomicBool = AtomicBool::new(false);
static mut FILTEROUT: AtomicBool = AtomicBool::new(false);
static LOG_LOCK: SpinNoIrqLock<()> = SpinNoIrqLock::new(());

static FLITER_LIST: &[&str] = &["/fd/3"];

pub fn print_in_color(args: fmt::Arguments, color_code: u8) {
    print!("\u{1B}[{}m{}\u{1B}[0m", color_code, args);
}

struct LogInterfaceImpl;

#[crate_interface::impl_interface]
impl LogInterface for LogInterfaceImpl {
    fn print_log(record: &log::Record) {
        let _guard = LOG_LOCK.lock();

        if !can_log() {
            return;
        }

        if can_filter() {
            let s = format!("{}", record.args());
            if FLITER_LIST
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
    unsafe { FILTEROUT.load(Ordering::Relaxed) }
}

#[allow(static_mut_refs)]
pub fn enable_filter() {
    log::debug!("Filter Enable");
    unsafe { FILTEROUT.store(true, Ordering::Relaxed) };
}

#[allow(static_mut_refs)]
pub fn disable_filter() {
    unsafe { FILTEROUT.store(false, Ordering::Relaxed) }
}
