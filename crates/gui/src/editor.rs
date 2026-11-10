//! Syntax editor logic: lines of a selection and the command under the cursor.

/// The 1-based line that holds the character at `char_index`.
pub fn line_of(text: &str, char_index: usize) -> u32 {
    1 + text.chars().take(char_index).filter(|&c| c == '\n').count() as u32
}

/// The byte where the character at `char_index` starts, or the end of the text.
pub fn byte_of(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(i, _)| i)
}

/// The lines a selection between two character positions covers, in either order. A selection
/// that ends at the very start of a line (after selecting whole lines) leaves that line out.
pub fn selected_lines(text: &str, a: usize, b: usize) -> (u32, u32) {
    let (start, end) = (a.min(b), a.max(b));
    let first = line_of(text, start);
    let mut last = line_of(text, end);
    let at_line_start = end > start && end > 0 && text.chars().nth(end - 1) == Some('\n');
    if at_line_start {
        last -= 1;
    }
    (first, last.max(first))
}

/// The line range widened to the whole commands it touches, so a cursor inside a command that
/// spans several lines runs that command. Lines with no command (the blank line under a command
/// just typed and followed by Enter) run the command above them, or else the one below.
/// Unchanged when the text does not split.
pub fn whole_commands(text: &str, (first, last): (u32, u32)) -> (u32, u32) {
    let Ok(commands) = statscraft_syntax::split_commands(text) else {
        return (first, last);
    };
    let touched = |c: &&statscraft_syntax::RawCommand| c.first_line <= last && c.last_line >= first;
    let nearest = || {
        let above = commands.iter().rev().find(|c| c.last_line < first);
        above.or_else(|| commands.iter().find(|c| c.first_line > last))
    };
    if !commands.iter().any(|c| touched(&c))
        && let Some(c) = nearest()
    {
        return (c.first_line, c.last_line);
    }
    commands
        .iter()
        .filter(touched)
        .fold((first, last), |(a, b), c| {
            (a.min(c.first_line), b.max(c.last_line))
        })
}

/// The command whose help fits the cursor line: the command that covers the line, else a
/// command name at the start of the line. None on a blank line or outside any command.
pub fn command_at(text: &str, line: u32) -> Option<String> {
    if let Ok(commands) = statscraft_syntax::split_commands(text)
        && let Some(command) = commands
            .iter()
            .find(|c| c.first_line <= line && line <= c.last_line)
    {
        let known = statscraft_syntax::help(Some(&command.name)).is_some();
        return known.then(|| command.name.clone());
    }
    // the text does not split (an open quote, say): try the words at the start of the line
    let words: Vec<&str> = text
        .lines()
        .nth(line as usize - 1)?
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(3)
        .collect();
    (1..=words.len()).rev().find_map(|n| {
        let name = words[..n].join(" ");
        statscraft_syntax::help(Some(&name)).map(|_| name.to_uppercase())
    })
}

/// Text appended to the editor: on a line of its own, separated from what is there.
pub fn append(text: &mut String, syntax: &str) {
    let trimmed = text.trim_end().len();
    text.truncate(trimmed);
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(syntax);
    text.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "GET FILE='a.sav'.\nFREQUENCIES\n  VARIABLES=x.\n\nCOMPUTE y = 1.\n";

    #[test]
    fn selections_map_to_whole_lines() {
        assert_eq!(line_of(SOURCE, 0), 1);
        assert_eq!(line_of(SOURCE, 18), 2);
        // from inside line 2 to inside line 3 (which starts at 30)
        assert_eq!(selected_lines(SOURCE, 20, 32), (2, 3));
        // reversed selection
        assert_eq!(selected_lines(SOURCE, 32, 20), (2, 3));
        // ending at the very start of line 3 leaves it out
        assert_eq!(selected_lines(SOURCE, 20, 30), (2, 2));
        // lines 2 and 3 selected whole, ending at the start of line 4
        let line4 = SOURCE.find("\n\n").unwrap() + 1;
        assert_eq!(selected_lines(SOURCE, 18, line4), (2, 3));
        // no selection: the cursor line
        assert_eq!(selected_lines(SOURCE, 20, 20), (2, 2));
    }

    #[test]
    fn a_selection_inside_a_command_runs_the_whole_command() {
        assert_eq!(whole_commands(SOURCE, (3, 3)), (2, 3));
        assert_eq!(whole_commands(SOURCE, (1, 2)), (1, 3));
        // the blank line 4 runs the command above it
        assert_eq!(whole_commands(SOURCE, (4, 4)), (2, 3));
        assert_eq!(whole_commands(SOURCE, (4, 5)), (4, 5));
        assert_eq!(whole_commands("\n\nfre x.", (1, 1)), (3, 3));
        assert_eq!(whole_commands("x = 'open\n", (1, 1)), (1, 1));
    }

    /// Typing `fre gender`, pressing Enter and then Run selection left the cursor on the empty
    /// line below, which held no command: the run stopped with "no command to run".
    #[test]
    fn short_forms_run_from_the_editor() {
        use crate::dialogs::testing::open;
        let tables = |source: &str, a: usize, b: usize| {
            let lines = whole_commands(source, selected_lines(source, a, b));
            let result = open("survey_basic.sav").run(source, Some(lines));
            let result = result.unwrap_or_else(|e| panic!("{source:?} at {a}..{b}: {e:?}"));
            result
                .commands
                .iter()
                .map(|c| c.tables.len())
                .sum::<usize>()
        };
        for source in [
            "fre gender",
            "fre gender.",
            "fre gender\n",
            "FRE gender.\r\n",
        ] {
            let end = source.chars().count();
            assert_eq!(
                tables(source, end, end),
                1,
                "{source:?} with the cursor at the end"
            );
            assert_eq!(
                tables(source, 0, 0),
                1,
                "{source:?} with the cursor at the start"
            );
            assert_eq!(tables(source, 0, end), 1, "{source:?} selected");
            // Run all
            let all = open("survey_basic.sav").run(source, None).unwrap();
            assert_eq!(all.commands[0].tables.len(), 1);
        }
        let source = "val lab gender 1 'M' 2 'F'.\nvar lab gender 'Sex'.\nfre gender\n";
        let end = source.chars().count();
        assert_eq!(tables(source, end, end), 1);
        assert_eq!(tables(source, 0, end), 1);
    }

    #[test]
    fn help_follows_the_command_under_the_cursor() {
        assert_eq!(command_at(SOURCE, 1).as_deref(), Some("GET"));
        assert_eq!(command_at(SOURCE, 3).as_deref(), Some("FREQUENCIES"));
        assert_eq!(command_at(SOURCE, 4), None);
        assert_eq!(command_at(SOURCE, 5).as_deref(), Some("COMPUTE"));
        // an unclosed quote stops splitting; the line start still names the command
        let broken = "variable labels x 'open\nfreq x.";
        assert_eq!(command_at(broken, 1).as_deref(), Some("VARIABLE LABELS"));
        assert_eq!(command_at("hello world.", 1), None);
    }

    #[test]
    fn pasted_syntax_goes_on_its_own_line() {
        let mut text = String::new();
        append(&mut text, "FREQUENCIES VARIABLES=x.");
        assert_eq!(text, "FREQUENCIES VARIABLES=x.\n");
        append(&mut text, "COMPUTE y = 1.");
        assert_eq!(text, "FREQUENCIES VARIABLES=x.\nCOMPUTE y = 1.\n");
        let mut text = "EXECUTE.\n\n\n".to_string();
        append(&mut text, "SAVE OUTFILE='a.sav'.");
        assert_eq!(text, "EXECUTE.\nSAVE OUTFILE='a.sav'.\n");
    }
}
