//! Result tables and charts as an Excel workbook, for the command line and the window alike.
//! The writer lives in the engine, which OUTPUT EXPORT also writes workbooks with.

pub use statscraft_engine::office::Item;
pub use statscraft_engine::xlsx::write_xlsx;
