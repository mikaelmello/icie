use crate::{ci, test::TestRun, util};
use std::fs;

#[derive(Debug, evscode::Configurable)]
enum HideBehaviour {
	#[evscode(name = "Always")]
	Always,
	#[evscode(name = "If any test failed")]
	IfAnyFailed,
	#[evscode(name = "Never")]
	Never,
}
impl HideBehaviour {
	fn should(&self, any_failed: bool) -> bool {
		match self {
			HideBehaviour::Always => true,
			HideBehaviour::IfAnyFailed => any_failed,
			HideBehaviour::Never => false,
		}
	}
}

#[evscode::config(description = "Fold AC in test view")]
static FOLD_AC: evscode::Config<HideBehaviour> = HideBehaviour::Never;

#[evscode::config(description = "Hide AC in test view")]
static HIDE_AC: evscode::Config<HideBehaviour> = HideBehaviour::Never;

pub fn render(tests: &[TestRun]) -> evscode::R<String> {
	Ok(format!(
		r#"
		<html>
			<head>
				<style>{css}</style>
				{material_icons}
				<script>{js}</script>
			</head>
			<body>
				<table class="test-table">
					{test_table}
				</table>
				<br/>
				<div id="new-container" class="new">
					<textarea id="new-input" class="new"></textarea>
					<textarea id="new-desired" class="new"></textarea>
					<div id="new-start" class="material-icons button new" onclick="new_start()">add</div>
					<div id="new-confirm" class="material-icons button new" onclick="new_confirm()">done</div>
				</div>
			</body>
		</html>
	"#,
		css = include_str!("./style.css"),
		material_icons = util::html_material_icons(),
		js = include_str!("./script.js"),
		test_table = render_test_table(tests)?
	))
}

fn render_test_table(tests: &[TestRun]) -> evscode::R<String> {
	let any_failed = tests.iter().any(|test| !test.success());
	let mut html = String::new();
	for test in tests {
		html += &render_test(test, any_failed)?;
	}
	Ok(html)
}

fn render_test(test: &TestRun, any_failed: bool) -> evscode::R<String> {
	if test.success() && HIDE_AC.get().should(any_failed) {
		return Ok(String::new());
	}
	let folded = test.success() && FOLD_AC.get().should(any_failed);
	Ok(format!(
		r#"
		<tr class="test-row {failed_flag}" data-in_path="{in_path}" data-out_raw="{out_raw}">
			{input}
			{out}
			{desired}
		</tr>
	"#,
		failed_flag = if !test.success() { "test-row-failed" } else { "" },
		in_path = test.in_path.display(),
		out_raw = html_attr_escape(&test.outcome.out),
		input = render_in_cell(test, folded)?,
		out = render_out_cell(test, folded)?,
		desired = render_desired_cell(test, folded)?
	))
}

fn render_in_cell(test: &TestRun, folded: bool) -> evscode::R<String> {
	Ok(render_cell("", &[ACTION_COPY, ACTION_EDIT], Data::raw(&fs::read_to_string(&test.in_path)?), None, folded))
}

fn render_out_cell(test: &TestRun, folded: bool) -> evscode::R<String> {
	use ci::test::Verdict::*;
	let outcome_class = match test.outcome.verdict {
		Accepted { alternative: false } => "test-good",
		Accepted { alternative: true } => "test-alt",
		WrongAnswer | RuntimeError | TimeLimitExceeded => "test-bad",
		IgnoredNoOut => "test-warn",
	};
	let mut actions = Vec::new();
	actions.push(ACTION_COPY);
	match test.outcome.verdict {
		Accepted { alternative: true } => actions.push(ACTION_DEL_ALT),
		WrongAnswer => actions.push(ACTION_SET_ALT),
		_ => (),
	};
	actions.push(ACTION_GDB);
	actions.push(ACTION_RR);
	let note = match test.outcome.verdict {
		Accepted { .. } | WrongAnswer => None,
		RuntimeError => Some("Runtime Error"),
		TimeLimitExceeded => Some("Time Limit Exceeded"),
		IgnoredNoOut => Some("Ignored"),
	};
	Ok(render_cell(
		outcome_class,
		&actions,
		Data::stdout_stderr(&test.outcome.out, &test.outcome.stderr),
		note,
		folded,
	))
}

fn render_desired_cell(test: &TestRun, folded: bool) -> evscode::R<String> {
	Ok(if test.out_path.exists() {
		render_cell("test-desired", &[ACTION_COPY, ACTION_EDIT], Data::raw(&fs::read_to_string(&test.out_path)?), None, folded)
	} else {
		render_cell("test-desired", &[ACTION_EDIT], Data::raw(""), Some("File does not exist"), folded)
	})
}

struct Action {
	onclick: &'static str,
	icon: &'static str,
	hint: &'static str,
}
const ACTION_COPY: Action = Action {
	onclick: "trigger_copy()",
	icon: "file_copy",
	hint: "Copy contents",
};
const ACTION_EDIT: Action = Action {
	onclick: "trigger_edit()",
	icon: "edit",
	hint: "Edit",
};
const ACTION_GDB: Action = Action {
	onclick: "trigger_gdb()",
	icon: "skip_previous",
	hint: "Debug in GDB",
};
const ACTION_RR: Action = Action {
	onclick: "trigger_rr()",
	icon: "fast_rewind",
	hint: "Debug in RR",
};
const ACTION_SET_ALT: Action = Action {
	onclick: "trigger_set_alt()",
	icon: "check",
	hint: "Set as an alternative correct output",
};
const ACTION_DEL_ALT: Action = Action {
	onclick: "trigger_del_alt()",
	icon: "close",
	hint: "Stop accepting this output",
};

#[evscode::config(description = "Max test lines displayed in test view. Lines after the limit will be replaced with an ellipsis. Set to 0 to denote no limit.")]
static MAX_TEST_LINES: evscode::Config<usize> = 0usize;

struct Data {
	html: String,
}

impl Data {
	fn raw(text: &str) -> Data {
		Data { html: html_escape(text.trim()) }
	}

	fn stdout_stderr(stdout: &str, stderr: &str) -> Data {
		let stdout = stdout.trim();
		let stderr = stderr.trim();
		let mut html = String::new();
		html += &format!("<div class=\"stderr\">{}</div>", html_escape(stderr));
		html += &html_escape(stdout);
		Data { html }
	}

	fn truncate_lines(&self) -> (String, usize) {
		let limit = MAX_TEST_LINES.get();
		let lines = self.html.split("<br/>").collect::<Vec<_>>();
		if *limit == 0 || lines.len() <= *limit + 1 {
			(self.html.clone(), lines.len())
		} else {
			(lines[..*limit].join("<br/>"), *limit + 1)
		}
	}
}

fn render_cell(class: &str, actions: &[Action], data: Data, note: Option<&str>, folded: bool) -> String {
	log::info!("data.html = {}", data.html);
	if folded {
		return format!(
			r#"
			<td class="test-cell {class} folded">
			</td>
		"#,
			class = class
		);
	}
	let note_div = if let Some(note) = note {
		format!(r#"<div class="test-note">{note}</div>"#, note = note)
	} else {
		String::new()
	};
	let mut action_list = String::new();
	for action in actions {
		action_list += &format!(
			r#"<div class="test-action material-icons" onclick="{}" title="{}">{}</div>"#,
			action.onclick, action.hint, action.icon
		);
	}
	let (data, lines) = data.truncate_lines();
	format!(
		r#"
		<td style="height: {lines_em}em; line-height: 1.1em;" class="test-cell {class}">
			<div class="test-actions">
				{action_list}
			</div>
			<div class="test-data">
				{data}
			</div>
			{note_div}
		</td>
	"#,
		lines_em = 1.1 * lines as f64,
		class = class,
		action_list = action_list,
		data = data,
		note_div = note_div
	)
}

fn html_escape(s: &str) -> String {
	translate(s, &[('\n', "<br/>"), ('&', "&amp;"), ('<', "&lt;"), ('>', "&gt;"), ('"', "&quot;"), ('\'', "&#39;")])
}
fn html_attr_escape(s: &str) -> String {
	translate(s, &[('&', "&amp;"), ('<', "&lt;"), ('>', "&gt;"), ('"', "&quot;"), ('\'', "&#39;")])
}
fn translate(s: &str, table: &[(char, &str)]) -> String {
	let mut buf = String::new();
	for c in s.chars() {
		match table.iter().find(|rule| rule.0 == c) {
			Some(rule) => buf += rule.1,
			_ => buf.push(c),
		}
	}
	buf
}