use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use std::{error::Error, io, time::{Duration, Instant}};

// --- Expression Parser Section (Shunting-yard Algorithm) ---

/// Returns the precedence of an operator.
fn precedence(op: char) -> u8 {
    match op {
        '+' | '-' => 1,
        '*' | '/' => 2,
        '^' => 3,
        _ => 0,
    }
}

/// Applies an operator to two numbers.
fn apply_op(op: char, b: f64, a: f64) -> Result<f64, &'static str> {
    match op {
        '+' => Ok(a + b),
        '-' => Ok(a - b),
        '*' => Ok(a * b),
        '/' => if b == 0.0 { Err("Division by zero") } else { Ok(a / b) },
        '^' => Ok(a.powf(b)),
        _ => Err("Unknown operator"),
    }
}

/// The main evaluation function that respects the order of operations.
fn evaluate(expression: &str) -> Result<f64, &'static str> {
    let mut values: Vec<f64> = Vec::new();
    let mut ops: Vec<char> = Vec::new();
    let mut chars = expression.chars().filter(|&c| !c.is_whitespace()).peekable();
    let mut last_was_op = true;

    while let Some(token) = chars.next() {
        match token {
            '0'..='9' | '.' => {
                let mut num_str = String::new();
                num_str.push(token);
                while let Some(&c) = chars.peek() {
                    if c.is_digit(10) || c == '.' {
                        num_str.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                let mut num: f64 = num_str.parse().map_err(|_| "Invalid number")?;
                
                if let Some('%') = chars.peek() {
                    if let Some(last_op) = ops.last() {
                        if *last_op == '+' || *last_op == '-' {
                            let prev_val = values.last().ok_or("Syntax error")?;
                            num = prev_val * (num / 100.0);
                        } else {
                            num /= 100.0;
                        }
                    } else {
                        num /= 100.0;
                    }
                    chars.next(); // Consume the '%'
                }
                
                values.push(num);
                last_was_op = false;
            }
            '(' => { ops.push('('); last_was_op = true; }
            ')' => {
                while let Some(op) = ops.pop() {
                    if op == '(' { break; }
                    let val2 = values.pop().ok_or("Syntax error")?;
                    let val1 = values.pop().ok_or("Syntax error")?;
                    values.push(apply_op(op, val2, val1)?);
                }
                last_was_op = false;
            }
            op @ ('+' | '*' | '/' | '^') => {
                while let Some(&top_op) = ops.last() {
                    if top_op != '(' && precedence(top_op) >= precedence(op) {
                        let val2 = values.pop().ok_or("Syntax error")?;
                        let val1 = values.pop().ok_or("Syntax error")?;
                        values.push(apply_op(ops.pop().unwrap(), val2, val1)?);
                    } else { break; }
                }
                ops.push(op);
                last_was_op = true;
            }
            '-' => {
                if last_was_op {
                    let mut num_str = String::from("-");
                    while let Some(&c) = chars.peek() {
                        if c.is_digit(10) || c == '.' { num_str.push(chars.next().unwrap()); } else { break; }
                    }
                    let mut num: f64 = num_str.parse().map_err(|_| "Invalid number")?;
                     if let Some('%') = chars.peek() {
                        num /= 100.0;
                        chars.next();
                    }
                    values.push(num);
                    last_was_op = false;
                } else {
                    while let Some(&top_op) = ops.last() {
                        if top_op != '(' && precedence(top_op) >= precedence('-') {
                            let val2 = values.pop().ok_or("Syntax error")?;
                            let val1 = values.pop().ok_or("Syntax error")?;
                            values.push(apply_op(ops.pop().unwrap(), val2, val1)?);
                        } else { break; }
                    }
                    ops.push('-');
                    last_was_op = true;
                }
            }
            _ => return Err("Invalid character"),
        }
    }

    while let Some(op) = ops.pop() {
        let val2 = values.pop().ok_or("Syntax error")?;
        let val1 = values.pop().ok_or("Syntax error")?;
        values.push(apply_op(op, val2, val1)?);
    }

    values.pop().ok_or("Syntax error")
}


// --- End of Parser Section ---

/// A struct for storing the color theme.
struct Theme {
    background: Color, display_bg: Color, border: Color, text: Color,
    num_button_fg: Color, op_button_fg: Color, num_button_bg: Color,
    op_button_bg: Color, equal_button_bg: Color, active_button_bg: Color,
}

impl Theme {
    fn default() -> Self {
        Theme {
            background: Color::Rgb(20, 20, 30), display_bg: Color::Rgb(50, 50, 60),
            border: Color::Rgb(80, 80, 90), text: Color::White,
            num_button_fg: Color::White, op_button_fg: Color::Rgb(20, 20, 30),
            num_button_bg: Color::Rgb(60, 70, 80), op_button_bg: Color::Rgb(255, 159, 67),
            equal_button_bg: Color::Rgb(255, 99, 132), active_button_bg: Color::White,
        }
    }
}

/// The main application struct.
struct App {
    display_value: String, is_result_displayed: bool, active_button: Option<(String, Instant)>,
    button_rects: Vec<(Rect, String)>, should_quit: bool, theme: Theme, last_op_duration: Option<Duration>,
}

impl App {
    fn new() -> App {
        App {
            display_value: String::from("0"), is_result_displayed: false, active_button: None,
            button_rects: Vec::new(), should_quit: false, theme: Theme::default(), last_op_duration: None,
        }
    }
    
    fn set_active_button(&mut self, label: &str) {
        self.active_button = Some((label.to_string(), Instant::now()));
    }
}

/// The logic executed when a button is clicked.
fn on_click(app: &mut App, value: &str) {
    app.set_active_button(value);
    
    match value {
        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "(" | ")" => {
            if app.is_result_displayed { app.display_value = String::from(value); app.is_result_displayed = false; }
            else if app.display_value == "0" { app.display_value = String::from(value); }
            else { app.display_value.push_str(value); }
        }
        "." => {
            let last_segment = app.display_value.split(&['+', '-', '*', '/', '^', '(', ')'][..]).last().unwrap_or("");
            if !last_segment.contains('.') { app.display_value.push('.'); }
        }
        "C" => { app.display_value = String::from("0"); app.is_result_displayed = false; app.last_op_duration = None; }
        "+/-" => {
             if let Some(last_num_start) = app.display_value.rfind(|c: char| !c.is_digit(10) && c != '.') {
                 let (before, after) = app.display_value.split_at(last_num_start + 1);
                 if after.starts_with('-') { app.display_value = format!("{}{}", before, &after[1..]); }
                 else { app.display_value = format!("{}-{}", before, after); }
             } else {
                 if app.display_value.starts_with('-') { app.display_value = app.display_value[1..].to_string(); }
                 else if app.display_value != "0" { app.display_value = format!("-{}", app.display_value); }
             }
        }
        "%" => {
            let last_char = app.display_value.chars().last().unwrap_or(' ');
            if last_char.is_digit(10) || last_char == ')' { app.display_value.push_str(value); }
        }
        "+" | "-" | "*" | "/" | "^" => {
            app.display_value = app.display_value.trim().to_string();
            app.display_value.push_str(&format!(" {} ", value));
            app.is_result_displayed = false;
        }
        "=" => {
            let start_time = Instant::now();
            let result = evaluate(&app.display_value);
            let duration = start_time.elapsed();
            app.last_op_duration = Some(duration);

            match result {
                Ok(res) => { app.display_value = format_result(res); app.is_result_displayed = true; }
                Err(e) => { app.display_value = e.to_string(); app.is_result_displayed = true; }
            }
        }
        _ => {}
    }
}

/// Handles the Backspace key press.
fn on_backspace(app: &mut App) {
    if app.is_result_displayed {
        app.display_value = String::from("0");
        app.is_result_displayed = false;
    } else if app.display_value.len() > 1 {
        let last_char = app.display_value.pop();
        // If the last character was a space, pop again to remove the operator
        if last_char == Some(' ') {
            app.display_value.pop();
        }
    } else {
        app.display_value = String::from("0");
    }
}


/// Formats the result, removing trailing zeros.
fn format_result(n: f64) -> String {
    if n.is_nan() { "Error".to_string() }
    else if n.fract() == 0.0 { format!("{:.0}", n) }
    else { format!("{:.8}", n).trim_end_matches('0').trim_end_matches('.').to_string() }
}


/// The main function of the program.
fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    if let Err(err) = res { println!("{err:?}"); }
    Ok(())
}

/// The main application loop: handles events and draws the UI.
fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;
        if let Some((_, time)) = app.active_button {
            if time.elapsed().as_millis() > 100 { app.active_button = None; }
        }
        if crossterm::event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.code == KeyCode::Char('q') => app.should_quit = true,
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(event::MouseButton::Left) => {
                    if let Some(label) = app.button_rects.iter().find_map(|(rect, label)| {
                        if rect.contains((mouse.column, mouse.row).into()) { Some(label.clone()) } else { None }
                    }) {
                        on_click(app, &label);
                    }
                },
                Event::Key(key) => {
                    match key.code {
                        KeyCode::Char(c @ ('0'..='9' | '(' | ')')) => on_click(app, &c.to_string()),
                        KeyCode::Char(c @ ('+' | '-' | '*' | '/' | '^' | '%')) => on_click(app, &c.to_string()),
                        KeyCode::Char('.') => on_click(app, "."),
                        KeyCode::Enter => on_click(app, "="),
                        KeyCode::Backspace => on_backspace(app),
                        KeyCode::Esc => on_click(app, "C"),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        if app.should_quit { return Ok(()); }
    }
}

/// The function that draws the entire UI.
fn ui(f: &mut Frame, app: &mut App) {
    app.button_rects.clear();
    let theme = &app.theme;
    f.render_widget(Block::default().bg(theme.background), f.size());
    let main_chunks = Layout::default().direction(Direction::Vertical).margin(1)
        .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.size());
    let time_text = if let Some(duration) = app.last_op_duration { format!("Last operation: {} µs", duration.as_micros()) } else { "Waiting for calculation...".to_string() };
    f.render_widget(Paragraph::new(time_text).style(Style::default().fg(theme.border)).alignment(Alignment::Right), main_chunks[0]);
    f.render_widget(Paragraph::new(app.display_value.as_str()).style(Style::default().fg(theme.text).bg(theme.display_bg)).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.border))).alignment(Alignment::Right), main_chunks[1]);
    f.render_widget(Paragraph::new(" Press 'q' to quit").style(Style::default().fg(theme.border)), main_chunks[3]);
    let button_definitions = [
        ("C", 0, 0, 1, 1), ("(", 1, 0, 1, 1), (")", 2, 0, 1, 1), ("/", 3, 0, 1, 1), ("%", 4, 0, 1, 1),
        ("7", 0, 1, 1, 1), ("8", 1, 1, 1, 1), ("9", 2, 1, 1, 1), ("*", 3, 1, 1, 1), ("^", 4, 1, 1, 1),
        ("4", 0, 2, 1, 1), ("5", 1, 2, 1, 1), ("6", 2, 2, 1, 1), ("-", 3, 2, 1, 1), ("+/-", 4, 2, 1, 1),
        ("1", 0, 3, 1, 1), ("2", 1, 3, 1, 1), ("3", 2, 3, 1, 1), ("+", 3, 3, 1, 2),
        ("0", 0, 4, 2, 1), (".", 2, 4, 1, 1), ("=", 4, 3, 1, 2),
    ];
    let rows = Layout::default().direction(Direction::Vertical).constraints([Constraint::Ratio(1, 5); 5]).split(main_chunks[2]);
    let mut cols_per_row = Vec::new();
    for row_area in rows.iter() { cols_per_row.push(Layout::default().direction(Direction::Horizontal).constraints([Constraint::Ratio(1, 5); 5]).split(*row_area)); }
    for (label, x, y, w, h) in button_definitions.iter() {
        let button_area = cols_per_row[*y as usize][*x as usize].union(cols_per_row[(*y + *h - 1) as usize][(*x + *w - 1) as usize]);
        app.button_rects.push((button_area, label.to_string()));
        let is_active = app.active_button.as_ref().map_or(false, |(l, _)| l == *label);
        let (fg_color, bg_color) = if is_active {
            (theme.op_button_fg, theme.active_button_bg)
        } else {
            match *label {
                "C" | "/" | "*" | "-" | "+" | "%" | "^" | "+/-" | "(" | ")" => (theme.op_button_fg, theme.op_button_bg),
                "=" => (theme.op_button_fg, theme.equal_button_bg),
                _ => (theme.num_button_fg, theme.num_button_bg),
            }
        };
        f.render_widget(Paragraph::new(*label).style(Style::default().fg(fg_color).bg(bg_color)).alignment(Alignment::Center).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.background))), button_area);
    }
}


// --- Test Suite ---
#[cfg(test)]
mod tests {
    use super::*;
    /// A helper function for comparing floating-point numbers.
    fn assert_float_eq(a: f64, b: f64) { assert!((a - b).abs() < 1e-9, "Expected {}, got {}", b, a); }
    #[test] fn test_simple_addition() { assert_float_eq(evaluate("5 + 3").unwrap(), 8.0); }
    #[test] fn test_simple_subtraction() { assert_float_eq(evaluate("10 - 4").unwrap(), 6.0); }
    #[test] fn test_simple_multiplication() { assert_float_eq(evaluate("7 * 3").unwrap(), 21.0); }
    #[test] fn test_simple_division() { assert_float_eq(evaluate("20 / 4").unwrap(), 5.0); }
    #[test] fn test_order_of_operations() { assert_float_eq(evaluate("5 + 2 * 3").unwrap(), 11.0); }
    #[test] fn test_parentheses() { assert_float_eq(evaluate("(5 + 2) * 3").unwrap(), 21.0); }
    #[test] fn test_floating_point() { assert_float_eq(evaluate("1.5 + 2.5").unwrap(), 4.0); }
    #[test] fn test_unary_minus() { assert_float_eq(evaluate("10 * -2").unwrap(), -20.0); }
    #[test] fn test_exponentiation() { assert_float_eq(evaluate("2 ^ 3").unwrap(), 8.0); }
    #[test]
    fn test_percentage() {
        assert_float_eq(evaluate("50%").unwrap(), 0.5);
        assert_float_eq(evaluate("200 + 10%").unwrap(), 220.0);
        assert_float_eq(evaluate("100 * 50%").unwrap(), 50.0);
        assert_float_eq(evaluate("100 - 25%").unwrap(), 75.0);
    }
    #[test] fn test_complex_expression() { assert_float_eq(evaluate("3 + 4 * 2 / ( 1 - 5 ) ^ 2").unwrap(), 3.5); }
    #[test] fn test_division_by_zero() { assert!(evaluate("10 / 0").is_err()); }
    #[test] fn test_syntax_error() { assert!(evaluate("5 * + 3").is_err()); }
}