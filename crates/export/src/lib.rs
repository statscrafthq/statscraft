//! Result tables and charts as an Excel workbook, and charts as pictures, for the command line
//! and the window alike. The workbook writer and the drawing live in the engine, which OUTPUT
//! EXPORT also writes workbooks and pictures with.

pub use statscraft_engine::office::Item;
pub use statscraft_engine::picture;
pub use statscraft_engine::xlsx::write_xlsx;
