use colored::{Color, Colorize};
use flexi_logger::{DeferredNow, style};

pub fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    write!(w, "{}   ", now.format("%-l:%M:%S%.3f%P"))?;

    let level = format!("{: <8}", record.level());
    write!(w, "{}", style(record.level()).paint(level))?;

    // write!(w, "{: <16}", thread::current().name().unwrap_or("<unnamed>"),)?;

    // Flexilogger only accepts a function pointer, not a closure.
    // TODO: output json + pino-pretty or something like that?
    let log_target = std::env::args().any(|arg| arg == "--log-target");

    if log_target {
        let target = record.target().to_string();

        let color = if target.starts_with("vaporeon") {
            Color::Green
        } else if target.starts_with("::") {
            Color::Blue
        } else {
            Color::Black
        };

        write!(w, "{:38}", target[..target.len().min(35)].color(color))?;
    }

    write!(w, "{}", record.args())?;
    Ok(())
}
