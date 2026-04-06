//! Calculator tool — evaluate math expressions.
//!
//! Uses a minimal recursive-descent parser to evaluate arithmetic
//! expressions. Supports: `+`, `-`, `*`, `/`, `%`, `^` (power),
//! unary minus, parentheses, and common math functions.

use async_trait::async_trait;
use ironclaw_core::{Tool, ToolError, ToolSchema};
use serde_json::{json, Value};

/// Evaluate math expressions without external dependencies.
pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Evaluate a mathematical expression. Supports +, -, *, /, %, ^ (power), \
         parentheses, and functions: sqrt, abs, sin, cos, tan, log, ln, ceil, floor, round."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            self.name(),
            self.description(),
            json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The math expression to evaluate, e.g. '(2 + 3) * 4 / sqrt(16)'"
                    }
                },
                "required": ["expression"]
            }),
        )
    }

    async fn invoke(&self, params: Value) -> Result<Value, ToolError> {
        let result: anyhow::Result<Value> = async {
            let expr = params["expression"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'expression' parameter"))?;

            let result = evaluate(expr)?;

            Ok(json!({
                "expression": expr,
                "result": result,
            }))
        }
        .await;
        result.map_err(Into::into)
    }
}

// ── Minimal expression evaluator ─────────────────────────────────────────

/// Evaluate a math expression string and return the result as f64.
fn evaluate(expr: &str) -> anyhow::Result<f64> {
    let tokens = tokenize(expr)?;
    let mut parser = Parser::new(&tokens);
    let result = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        anyhow::bail!("Unexpected token at position {}", parser.pos);
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
    Ident(String),
}

fn tokenize(input: &str) -> anyhow::Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let n: f64 = s
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid number: {s}"))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                // Check for named constants
                match name.as_str() {
                    "pi" | "PI" => tokens.push(Token::Number(std::f64::consts::PI)),
                    "e" | "E" => tokens.push(Token::Number(std::f64::consts::E)),
                    _ => tokens.push(Token::Ident(name)),
                }
            }
            c => anyhow::bail!("Unexpected character: '{c}'"),
        }
    }

    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// expr = term (('+' | '-') term)*
    fn parse_expr(&mut self) -> anyhow::Result<f64> {
        let mut left = self.parse_term()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Plus => {
                    self.advance();
                    left += self.parse_term()?;
                }
                Token::Minus => {
                    self.advance();
                    left -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// term = power (('*' | '/' | '%') power)*
    fn parse_term(&mut self) -> anyhow::Result<f64> {
        let mut left = self.parse_power()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Star => {
                    self.advance();
                    left *= self.parse_power()?;
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        anyhow::bail!("Division by zero");
                    }
                    left /= right;
                }
                Token::Percent => {
                    self.advance();
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        anyhow::bail!("Modulo by zero");
                    }
                    left %= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// power = unary ('^' power)?  (right-associative)
    fn parse_power(&mut self) -> anyhow::Result<f64> {
        let base = self.parse_unary()?;
        if let Some(Token::Caret) = self.peek() {
            self.advance();
            let exp = self.parse_power()?;
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    /// unary = '-' unary | primary
    fn parse_unary(&mut self) -> anyhow::Result<f64> {
        if let Some(Token::Minus) = self.peek() {
            self.advance();
            Ok(-self.parse_unary()?)
        } else {
            self.parse_primary()
        }
    }

    /// primary = Number | '(' expr ')' | func '(' expr (',' expr)* ')'
    fn parse_primary(&mut self) -> anyhow::Result<f64> {
        match self.advance().cloned() {
            Some(Token::Number(n)) => Ok(n),
            Some(Token::LParen) => {
                let val = self.parse_expr()?;
                match self.advance() {
                    Some(Token::RParen) => Ok(val),
                    _ => anyhow::bail!("Expected closing parenthesis"),
                }
            }
            Some(Token::Ident(name)) => {
                // Must be followed by '('
                match self.advance() {
                    Some(Token::LParen) => {}
                    _ => anyhow::bail!("Expected '(' after function name '{name}'"),
                }
                let arg = self.parse_expr()?;
                // Some functions take 2 args
                let result = match name.as_str() {
                    "sqrt" => {
                        self.expect_rparen()?;
                        arg.sqrt()
                    }
                    "abs" => {
                        self.expect_rparen()?;
                        arg.abs()
                    }
                    "sin" => {
                        self.expect_rparen()?;
                        arg.sin()
                    }
                    "cos" => {
                        self.expect_rparen()?;
                        arg.cos()
                    }
                    "tan" => {
                        self.expect_rparen()?;
                        arg.tan()
                    }
                    "ln" => {
                        self.expect_rparen()?;
                        arg.ln()
                    }
                    "log" => {
                        // log(x) = log10, log(base, x) = log_base(x)
                        if let Some(Token::Comma) = self.peek() {
                            self.advance();
                            let x = self.parse_expr()?;
                            self.expect_rparen()?;
                            x.log(arg)
                        } else {
                            self.expect_rparen()?;
                            arg.log10()
                        }
                    }
                    "ceil" => {
                        self.expect_rparen()?;
                        arg.ceil()
                    }
                    "floor" => {
                        self.expect_rparen()?;
                        arg.floor()
                    }
                    "round" => {
                        self.expect_rparen()?;
                        arg.round()
                    }
                    "min" => match self.peek() {
                        Some(Token::Comma) => {
                            self.advance();
                            let b = self.parse_expr()?;
                            self.expect_rparen()?;
                            arg.min(b)
                        }
                        _ => anyhow::bail!("min() requires 2 arguments"),
                    },
                    "max" => match self.peek() {
                        Some(Token::Comma) => {
                            self.advance();
                            let b = self.parse_expr()?;
                            self.expect_rparen()?;
                            arg.max(b)
                        }
                        _ => anyhow::bail!("max() requires 2 arguments"),
                    },
                    _ => anyhow::bail!("Unknown function: {name}"),
                };
                Ok(result)
            }
            Some(t) => anyhow::bail!("Unexpected token: {t:?}"),
            None => anyhow::bail!("Unexpected end of expression"),
        }
    }

    fn expect_rparen(&mut self) -> anyhow::Result<()> {
        match self.advance() {
            Some(Token::RParen) => Ok(()),
            _ => anyhow::bail!("Expected closing parenthesis"),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        assert_eq!(evaluate("2 + 3").unwrap(), 5.0);
        assert_eq!(evaluate("10 - 4").unwrap(), 6.0);
        assert_eq!(evaluate("3 * 7").unwrap(), 21.0);
        assert_eq!(evaluate("15 / 5").unwrap(), 3.0);
        assert_eq!(evaluate("7 % 3").unwrap(), 1.0);
    }

    #[test]
    fn operator_precedence() {
        assert_eq!(evaluate("2 + 3 * 4").unwrap(), 14.0);
        assert_eq!(evaluate("(2 + 3) * 4").unwrap(), 20.0);
    }

    #[test]
    fn power_right_associative() {
        assert_eq!(evaluate("2 ^ 3").unwrap(), 8.0);
        // 2^(3^2) = 2^9 = 512
        assert_eq!(evaluate("2 ^ 3 ^ 2").unwrap(), 512.0);
    }

    #[test]
    fn unary_minus() {
        assert_eq!(evaluate("-5").unwrap(), -5.0);
        assert_eq!(evaluate("-(3 + 4)").unwrap(), -7.0);
        assert_eq!(evaluate("2 * -3").unwrap(), -6.0);
    }

    #[test]
    fn functions() {
        assert_eq!(evaluate("sqrt(16)").unwrap(), 4.0);
        assert_eq!(evaluate("abs(-5)").unwrap(), 5.0);
        assert_eq!(evaluate("ceil(1.2)").unwrap(), 2.0);
        assert_eq!(evaluate("floor(1.8)").unwrap(), 1.0);
        assert_eq!(evaluate("round(1.5)").unwrap(), 2.0);
        assert_eq!(evaluate("min(3, 7)").unwrap(), 3.0);
        assert_eq!(evaluate("max(3, 7)").unwrap(), 7.0);
    }

    #[test]
    fn constants() {
        let pi = evaluate("pi").unwrap();
        assert!((pi - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn division_by_zero() {
        assert!(evaluate("1 / 0").is_err());
    }

    #[test]
    fn schema_is_valid() {
        let tool = CalculatorTool;
        assert_eq!(tool.name(), "calculator");
        let schema = tool.schema();
        assert!(schema.parameters["properties"]["expression"].is_object());
    }

    #[tokio::test]
    async fn invoke_returns_result() {
        let tool = CalculatorTool;
        let result = tool
            .invoke(json!({"expression": "(2 + 3) * 4"}))
            .await
            .unwrap();
        assert_eq!(result["result"], 20.0);
    }
}
