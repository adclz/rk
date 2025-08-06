#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateAndTimeError {
    InvalidYear(ErrorKind),
    InvalidMonth(ErrorKind),
    InvalidDay(ErrorKind),
}

impl DateAndTimeError {
    pub fn to_diag(&self) -> auto_lsp::lsp_types::Diagnostic {
        let (message, range) = match self {
            DateAndTimeError::InvalidYear(kind) => ("Invalid year".to_string(), 0..0),
            DateAndTimeError::InvalidMonth(kind) => ("Invalid month".to_string(), 0..0),
            DateAndTimeError::InvalidDay(kind) => ("Invalid day".to_string(), 0..0),
        };
        auto_lsp::lsp_types::Diagnostic {
            range: auto_lsp::lsp_types::Range {
                start: auto_lsp::lsp_types::Position {
                    line: 0,
                    character: range.start,
                },
                end: auto_lsp::lsp_types::Position {
                    line: 0,
                    character: range.end,
                },
            },
            message,
            severity: Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    RangeError(Range<usize>),
    Syntax(Range<usize>),
    Missing(Range<usize>),
    MaxValue(Range<usize>),
    MinValue(Range<usize>),
    InvalidForMonth(Range<usize>),
}

pub fn check_date(_db: &dyn crate::BaseDatabase, text: &str) -> Result<(), DateAndTimeError> {
    let parts: Vec<_> = text.split('-').collect();

    // Year
    let year_part = parts
        .first()
        .ok_or(DateAndTimeError::InvalidYear(ErrorKind::Missing(Range {
            start: 0,
            end: 1,
        })))?;
    let year_span = span_in_text(text, year_part);
    if year_part.len() != 4 {
        return Err(DateAndTimeError::InvalidYear(ErrorKind::RangeError(
            year_span,
        )));
    }
    let year: u16 = year_part
        .parse()
        .map_err(|_| DateAndTimeError::InvalidYear(ErrorKind::Syntax(year_span)))?;

    // Month
    let month_part = parts
        .get(1)
        .ok_or(DateAndTimeError::InvalidMonth(ErrorKind::Missing(Range {
            start: 5,
            end: 6,
        })))?;
    let month_span = span_in_text(text, month_part);
    if month_part.len() != 2 {
        return Err(DateAndTimeError::InvalidMonth(ErrorKind::RangeError(
            month_span,
        )));
    }
    let month: u8 = month_part
        .parse()
        .map_err(|_| DateAndTimeError::InvalidMonth(ErrorKind::Syntax(month_span.clone())))?;
    if !(1..=12).contains(&month) {
        return Err(if month == 0 {
            DateAndTimeError::InvalidMonth(ErrorKind::MinValue(month_span))
        } else {
            DateAndTimeError::InvalidMonth(ErrorKind::MaxValue(month_span))
        });
    }

    // Day
    let day_part = parts
        .get(2)
        .ok_or(DateAndTimeError::InvalidDay(ErrorKind::Missing(Range {
            start: 8,
            end: 10,
        })))?;
    let day_span = span_in_text(text, day_part);
    if day_part.len() != 2 {
        return Err(DateAndTimeError::InvalidDay(ErrorKind::RangeError(
            day_span,
        )));
    }
    let day: u8 = day_part
        .parse()
        .map_err(|_| DateAndTimeError::InvalidDay(ErrorKind::Syntax(day_span.clone())))?;

    // Validate day for given month
    let max_day = max_day_of_month(year, month);
    if day == 0 {
        return Err(DateAndTimeError::InvalidDay(ErrorKind::MinValue(day_span)));
    }
    if day > max_day {
        return Err(DateAndTimeError::InvalidDay(ErrorKind::InvalidForMonth(
            day_span,
        )));
    }

    Ok(())
}

fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn max_day_of_month(year: u16, month: u8) -> u8 {
    match month {
        1 => 31,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => unreachable!(),
    }
}

/// Finds the byte span of a specific part inside the full string.
fn span_in_text<'a>(full: &'a str, part: &'a str) -> Range<usize> {
    let start = full.find(part).unwrap_or(0);
    start..start + part.len()
}
