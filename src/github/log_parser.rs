use crate::models::JobLogStep;

/// Strip ANSI escape codes from log text.
/// Handles CSI sequences (ESC [ ... letter) and also strips \r.
pub(crate) fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we find a terminating ASCII letter
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else if c == '\r' {
            // Strip carriage returns
        } else {
            result.push(c);
        }
    }
    result
}

/// Parse job log text into per-step segments using ##[group]/##[endgroup] markers.
/// Also strips timestamp prefixes (e.g., "2024-01-01T12:00:00.0000000Z ") from each line.
pub(crate) fn parse_step_logs(log_text: &str) -> Vec<JobLogStep> {
    let mut steps: Vec<JobLogStep> = Vec::new();
    let mut current_name = String::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut in_group = false;

    for line in log_text.lines() {
        // Strip timestamp prefix: "2024-01-01T12:00:00.0000000Z " (28-char ISO + space)
        let content = if line.len() > 28 && line.as_bytes()[4] == b'-' && line.as_bytes()[27] == b'Z' {
            line[29..].trim_start()
        } else {
            line
        };

        // Check for ##[group] marker
        if content.starts_with("##[group]") {
            // Save any pending lines
            if in_group && !current_lines.is_empty() {
                steps.push(JobLogStep {
                    name: current_name.clone(),
                    log: current_lines.join("\n"),
                });
                current_lines.clear();
            } else if !in_group && !current_lines.is_empty() {
                steps.push(JobLogStep {
                    name: "Preamble".into(),
                    log: current_lines.join("\n"),
                });
                current_lines.clear();
            }
            current_name = content[9..].to_string();
            in_group = true;
            continue;
        }

        // Check for ##[endgroup] marker
        if content.starts_with("##[endgroup]") {
            if in_group {
                steps.push(JobLogStep {
                    name: current_name.clone(),
                    log: current_lines.join("\n"),
                });
                current_lines.clear();
                in_group = false;
            }
            continue;
        }

        // Strip other ## directives but keep content
        let clean = if content.starts_with("##[debug]") {
            &content[9..]
        } else if content.starts_with("##[warning]") {
            &content[11..]
        } else if content.starts_with("##[error]") {
            &content[9..]
        } else if content.starts_with("##[command]") {
            content[11..].trim_start()
        } else {
            content
        };

        current_lines.push(clean.to_string());
    }

    // Flush remaining lines
    if !current_lines.is_empty() {
        let name = if in_group {
            current_name
        } else if steps.is_empty() {
            "Full Log".into()
        } else {
            "Post".into()
        };
        steps.push(JobLogStep {
            name,
            log: current_lines.join("\n"),
        });
    }

    // If no steps at all, return a single empty entry
    if steps.is_empty() {
        steps.push(JobLogStep {
            name: "Full Log".into(),
            log: String::new(),
        });
    }

    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_basic() {
        let input = "\x1b[32mHello\x1b[0m World";
        assert_eq!(strip_ansi(input), "Hello World");
    }

    #[test]
    fn test_strip_ansi_no_codes() {
        let input = "Just plain text";
        assert_eq!(strip_ansi(input), "Just plain text");
    }

    #[test]
    fn test_strip_ansi_carriage_return() {
        let input = "line1\r\nline2\r";
        assert_eq!(strip_ansi(input), "line1\nline2");
    }

    #[test]
    fn test_strip_ansi_multiple_codes() {
        let input = "\x1b[1m\x1b[31mERROR\x1b[0m: \x1b[33mwarning\x1b[0m";
        assert_eq!(strip_ansi(input), "ERROR: warning");
    }

    #[test]
    fn test_strip_ansi_complex_csi() {
        // 256-color and RGB color codes
        let input = "\x1b[38;5;196mRed\x1b[0m \x1b[48;2;255;128;0mOrange\x1b[0m";
        assert_eq!(strip_ansi(input), "Red Orange");
    }

    #[test]
    fn test_strip_ansi_empty() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn test_strip_ansi_only_codes() {
        let input = "\x1b[32m\x1b[0m\x1b[1m\x1b[0m";
        assert_eq!(strip_ansi(input), "");
    }

    #[test]
    fn test_strip_ansi_preserves_newlines() {
        let input = "\x1b[32mline1\x1b[0m\nline2\n\x1b[31mline3\x1b[0m";
        assert_eq!(strip_ansi(input), "line1\nline2\nline3");
    }

    #[test]
    fn test_strip_ansi_mixed_cr_lf() {
        let input = "a\r\nb\rc\n";
        assert_eq!(strip_ansi(input), "a\nbc\n");
    }

    #[test]
    fn test_parse_step_logs_with_groups() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Set up job\n\
                    2024-01-01T12:00:01.0000000Z Preparing environment\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]\n\
                    2024-01-01T12:00:03.0000000Z ##[group]Run tests\n\
                    2024-01-01T12:00:04.0000000Z test passed\n\
                    2024-01-01T12:00:05.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Set up job");
        assert_eq!(steps[0].log, "Preparing environment");
        assert_eq!(steps[1].name, "Run tests");
        assert_eq!(steps[1].log, "test passed");
    }

    #[test]
    fn test_parse_step_logs_no_groups() {
        let log = "2024-01-01T12:00:00.0000000Z line 1\n\
                    2024-01-01T12:00:01.0000000Z line 2";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Full Log");
        assert!(steps[0].log.contains("line 1"));
        assert!(steps[0].log.contains("line 2"));
    }

    #[test]
    fn test_parse_step_logs_with_preamble() {
        let log = "2024-01-01T12:00:00.0000000Z preamble line\n\
                    2024-01-01T12:00:01.0000000Z ##[group]Step 1\n\
                    2024-01-01T12:00:02.0000000Z step content\n\
                    2024-01-01T12:00:03.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Preamble");
        assert_eq!(steps[0].log, "preamble line");
        assert_eq!(steps[1].name, "Step 1");
        assert_eq!(steps[1].log, "step content");
    }

    #[test]
    fn test_parse_step_logs_strips_directives() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Test\n\
                    2024-01-01T12:00:01.0000000Z ##[command]echo hello\n\
                    2024-01-01T12:00:02.0000000Z ##[warning]something\n\
                    2024-01-01T12:00:03.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Test");
        assert!(steps[0].log.contains("echo hello"));
        assert!(steps[0].log.contains("something"));
    }

    #[test]
    fn test_parse_step_logs_empty() {
        let steps = parse_step_logs("");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Full Log");
    }

    #[test]
    fn test_parse_step_logs_post_section() {
        // Lines after the last endgroup get a "Post" name
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Build\n\
                    2024-01-01T12:00:01.0000000Z building...\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]\n\
                    2024-01-01T12:00:03.0000000Z cleanup line";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Build");
        assert_eq!(steps[1].name, "Post");
        assert!(steps[1].log.contains("cleanup line"));
    }

    #[test]
    fn test_parse_step_logs_nested_groups() {
        // Consecutive groups without endgroup — second group closes first
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Step A\n\
                    2024-01-01T12:00:01.0000000Z a content\n\
                    2024-01-01T12:00:02.0000000Z ##[group]Step B\n\
                    2024-01-01T12:00:03.0000000Z b content\n\
                    2024-01-01T12:00:04.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Step A");
        assert!(steps[0].log.contains("a content"));
        assert_eq!(steps[1].name, "Step B");
        assert!(steps[1].log.contains("b content"));
    }

    #[test]
    fn test_parse_step_logs_debug_directive() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Test\n\
                    2024-01-01T12:00:01.0000000Z ##[debug]debug message\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert!(steps[0].log.contains("debug message"));
    }

    #[test]
    fn test_parse_step_logs_error_directive() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Build\n\
                    2024-01-01T12:00:01.0000000Z ##[error]compilation failed\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert!(steps[0].log.contains("compilation failed"));
    }

    #[test]
    fn test_parse_step_logs_no_timestamp() {
        // Lines without timestamp prefix should still be parsed
        let log = "##[group]Direct Step\nsome output\n##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Direct Step");
        assert!(steps[0].log.contains("some output"));
    }

    #[test]
    fn test_parse_step_logs_multiline_content() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Multi\n\
                    2024-01-01T12:00:01.0000000Z line 1\n\
                    2024-01-01T12:00:02.0000000Z line 2\n\
                    2024-01-01T12:00:03.0000000Z line 3\n\
                    2024-01-01T12:00:04.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        let lines: Vec<&str> = steps[0].log.lines().collect();
        assert_eq!(lines.len(), 3);
    }
}
