use crate::print;
use core::fmt;
use logger::LogInterface;

pub fn print_in_color(args: fmt::Arguments, color_code: u8) {
    print!("\u{1B}[{}m{}\u{1B}[0m", color_code, args);
}

struct LogInterfaceImpl;

#[crate_interface::impl_interface]
impl LogInterface for LogInterfaceImpl {
    fn print_log(record: &log::Record) {
        print_in_color(
            format_args!(
                "[{:>5}][{}:{}] {}\n",
                record.level(),
                record.file().unwrap(),
                record.line().unwrap(),
                record.args()
            ),
            logger::level2color(record.level()),
        )
    }
}
