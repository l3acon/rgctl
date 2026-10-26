//! Content-based disambiguation for `.m` (Objective-C vs MATLAB).

/// Inspect up to the first 4 KiB of a `.m` file and return `"matlab"` or `"objc"`.
///
/// MATLAB wins when the prefix contains MATLAB markers (`%` comments, `function` /
/// `classdef` headers, or script matrix/elementwise idioms) and does not contain
/// Objective-C / C preprocessor markers. Otherwise defaults to `"objc"`.
pub fn sniff_objc_vs_matlab(source: &[u8]) -> &'static str {
    let prefix = &source[..source.len().min(4096)];
    let text = String::from_utf8_lossy(prefix);
    let mut has_matlab = false;
    let mut has_objc = false;
    let mut in_c_block_comment = false;

    for raw in text.lines() {
        let mut line = raw.trim_start();
        if line.is_empty() {
            continue;
        }

        // Track C/Objective-C `/* ... */` block comments so words like `function`
        // inside an Objective-C license/header comment never masquerade as MATLAB.
        if in_c_block_comment {
            if let Some(end_idx) = line.find("*/") {
                in_c_block_comment = false;
                line = line[end_idx + 2..].trim_start();
                if line.is_empty() {
                    continue;
                }
            } else {
                continue;
            }
        }
        while let Some(start_idx) = line.find("/*") {
            has_objc = true;
            if let Some(rel_end) = line[start_idx + 2..].find("*/") {
                let after = &line[start_idx + 2 + rel_end + 2..];
                line = after.trim_start();
            } else {
                in_c_block_comment = true;
                line = &line[..start_idx];
                break;
            }
        }
        if line.is_empty() || line.starts_with("//") {
            if raw.trim_start().starts_with("//") {
                has_objc = true;
            }
            continue;
        }

        if line.starts_with("#import")
            || line.starts_with("#include")
            || line.starts_with("#pragma")
            || line.starts_with("#ifdef")
            || line.starts_with("#ifndef")
            || line.starts_with("@interface")
            || line.starts_with("@implementation")
            || line.starts_with("@protocol")
            || line.starts_with("@property")
            || line.starts_with("@synthesize")
            || line.starts_with("@autoreleasepool")
        {
            has_objc = true;
        }

        if line.starts_with('%')
            || has_keyword_boundary(line, "function")
            || has_keyword_boundary(line, "Function")
            || has_keyword_boundary(line, "classdef")
            || has_keyword_boundary(line, "Classdef")
            || line == "end"
            || line.starts_with("end;")
            || line.contains(".*")
            || line.contains("./")
            || line.contains(".^")
            || line.starts_with("disp(")
            || line.starts_with("fprintf(")
            || line.starts_with("linspace(")
        {
            has_matlab = true;
        }
    }

    if has_matlab && !has_objc {
        "matlab"
    } else {
        "objc"
    }
}

fn has_keyword_boundary(line: &str, kw: &str) -> bool {
    let Some(rest) = line.strip_prefix(kw) else {
        return false;
    };
    rest.starts_with(|c: char| c.is_whitespace() || c == '[' || c == '(')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matlab_function_header() {
        assert_eq!(
            sniff_objc_vs_matlab(b"function [y] = filt(x)\ny = x;\nend\n"),
            "matlab"
        );
    }

    #[test]
    fn matlab_classdef() {
        assert_eq!(
            sniff_objc_vs_matlab(b"% Solver\nclassdef Solver\nend\n"),
            "matlab"
        );
    }

    #[test]
    fn matlab_headerless_script() {
        assert_eq!(
            sniff_objc_vs_matlab(b"x = linspace(0, 1, 100);\ny = x .^ 2;\ndisp(y);\n"),
            "matlab"
        );
    }

    #[test]
    fn objc_import() {
        assert_eq!(
            sniff_objc_vs_matlab(b"#import <Foundation/Foundation.h>\n@interface Foo\n@end\n"),
            "objc"
        );
    }

    #[test]
    fn objc_with_comment_mentioning_function_stays_objc() {
        let src = b"/*\n * function helper for view controller\n */\n@implementation Foo\n@end\n";
        assert_eq!(sniff_objc_vs_matlab(src), "objc");
    }

    #[test]
    fn c_identifier_starting_with_function_prefix_stays_objc() {
        let src = b"int function_pointer = 0;\n";
        assert_eq!(sniff_objc_vs_matlab(src), "objc");
    }

    #[test]
    fn empty_defaults_to_objc() {
        assert_eq!(sniff_objc_vs_matlab(b""), "objc");
    }
}
