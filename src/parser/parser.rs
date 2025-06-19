use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, digit1, line_ending, multispace0, space0, space1},
    combinator::{map, map_res, opt, recognize},
    error::Error,
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};

type ParseResult<'a, T> = IResult<&'a str, T, Error<&'a str>>;

const KEYWORDS: &[&str] = &[
    "if",
    "in",
    "else",
    "def",
    "while",
    "for",
    "val",
    "var",
    "return",
    "Ok",
    "Err",
    "Just",
    "Nothing",
    "unwrap",
    "tryUnwrap",
    "isNothing",
    "isError",
    "and",
    "or",
    "not",
    "True",
    "False",
];

use crate::ir::ast::Function;
use crate::ir::ast::Type;
use crate::ir::ast::{Expression, Name, Statement, ValueConstructor};

fn identifier(input: &str) -> IResult<&str, Name> {
    let (input, id) = take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)?;

    if KEYWORDS.contains(&id) {
        return Err(nom::Err::Error(Error {
            input,
            code: nom::error::ErrorKind::Tag,
        }));
    }

    Ok((input, id.to_string()))
}

// Parse integer literals
fn integer(input: &str) -> IResult<&str, Expression> {
    map_res(
        pair(opt(preceded(space0, char('-'))), preceded(space0, digit1)),
        |(sign, digits): (Option<char>, &str)| {
            digits.parse::<i32>().map(|num| {
                if sign.is_some() {
                    Expression::CInt(-num)
                } else {
                    Expression::CInt(num)
                }
            })
        },
    )(input)
}

//term parser for arithmetic
fn term(input: &str) -> ParseResult<Expression> {
    let (mut input, mut expr) = factor(input)?;

    loop {
        let op_result = delimited::<_, _, _, _, Error<&str>, _, _, _>(
            space0::<&str, Error<&str>>,
            alt((tag("*"), tag("/"))),
            space0::<&str, Error<&str>>,
        )(input);

        match op_result {
            Ok((new_input, op)) => {
                let (newer_input, factor2) = factor(new_input)?;
                expr = match op {
                    "*" => Expression::Mul(Box::new(expr), Box::new(factor2)),
                    "/" => Expression::Div(Box::new(expr), Box::new(factor2)),
                    _ => unreachable!(),
                };
                input = newer_input;
            }
            Err(_) => break,
        }
    }

    Ok((input, expr))
}

//expression parser to include if statements
fn statement(input: &str) -> IResult<&str, Statement> {
    let (input, _) = space0(input)?;
    alt((
        function_def,
        if_statement,
        for_statement,
        return_statement,
        assignment,
        declaration,
        adt_declaration,  // Add ADT declaration
        match_expression, // Add pattern matching
    ))(input)
}

// Parse basic expressions
fn expression(input: &str) -> IResult<&str, Expression> {
    alt((
        boolean_expression,
        comparison_expression,
        arithmetic_expression,
        real,
        integer,
        ok_expression,
        err_expression,
        just_expression,
        nothing_expression,
        unwrap_expression,
        tryunwrap_expression,
        iserror_expression,
        isnothing_expression,
        string,
        map(identifier, Expression::Var),
    ))(input)
}

// Parse arithmetic operators (unused)
//fn operator(input: &str) -> IResult<&str, &str> {
//alt((tag("+"), tag("-"), tag("*"), tag("/")))(input)
//}

// Add comparison operator parsing
fn comparison_operator(input: &str) -> IResult<&str, &str> {
    alt((
        tag("=="),
        tag("!="),
        tag(">="),
        tag("<="),
        tag(">"),
        tag("<"),
    ))(input)
}

// Update expression to handle comparisons
fn comparison_expression(input: &str) -> IResult<&str, Expression> {
    let (input, left) = term(input)?;
    let (input, _) = space0(input)?;
    let (input, op) = comparison_operator(input)?;
    let (input, _) = space0(input)?;
    let (input, right) = term(input)?;
    Ok((
        input,
        match op {
            ">" => Expression::GT(Box::new(left), Box::new(right)),
            "<" => Expression::LT(Box::new(left), Box::new(right)),
            ">=" => Expression::GTE(Box::new(left), Box::new(right)),
            "<=" => Expression::LTE(Box::new(left), Box::new(right)),
            "==" => Expression::EQ(Box::new(left), Box::new(right)),
            _ => unreachable!(),
        },
    ))
}

// Parse expressions with operator precedence
fn arithmetic_expression(input: &str) -> ParseResult<Expression> {
    let (mut input, mut expr) = term(input)?;

    loop {
        let op_result = delimited::<_, _, _, _, Error<&str>, _, _, _>(
            space0::<&str, Error<&str>>,
            alt((tag("+"), tag("-"))),
            space0::<&str, Error<&str>>,
        )(input);

        match op_result {
            Ok((new_input, op)) => {
                let (newer_input, term2) = term(new_input)?;
                expr = match op {
                    "+" => Expression::Add(Box::new(expr), Box::new(term2)),
                    "-" => Expression::Sub(Box::new(expr), Box::new(term2)),
                    _ => unreachable!(),
                };
                input = newer_input;
            }
            Err(_) => break,
        }
    }

    Ok((input, expr))
}

// Add to imports
use nom::character::complete::char as char_parser;

// Parse boolean literals
fn boolean(input: &str) -> IResult<&str, Expression> {
    alt((
        map(tag("True"), |_| Expression::CTrue),
        map(tag("False"), |_| Expression::CFalse),
    ))(input)
}

// Parse real numbers
fn real(input: &str) -> IResult<&str, Expression> {
    map_res(
        recognize(tuple((opt(char('-')), digit1, char('.'), digit1))),
        |num_str: &str| num_str.parse::<f64>().map(Expression::CReal),
    )(input)
}

// Parse strings
fn string(input: &str) -> IResult<&str, Expression> {
    delimited(
        char_parser('"'),
        map(take_while(|c| c != '"'), |s: &str| {
            Expression::CString(s.to_string())
        }),
        char_parser('"'),
    )(input)
}

fn ok_expression(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("Ok")(input)?;
    let (input, _) = space0(input)?;
    let (input, expr) = delimited(
        tuple((char('('), space0)),
        expression,
        tuple((space0, char(')'))),
    )(input)?;

    Ok((input, Expression::COk(Box::new(expr))))
}

fn err_expression(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("Err")(input)?;
    let (input, _) = space0(input)?;
    let (input, expr) = delimited(
        tuple((char('('), space0)),
        expression,
        tuple((space0, char(')'))),
    )(input)?;

    Ok((input, Expression::CErr(Box::new(expr))))
}

fn just_expression(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("Just")(input)?;
    let (input, _) = space0(input)?;
    let (input, expr) = delimited(
        tuple((char('('), space0)),
        expression,
        tuple((space0, char(')'))),
    )(input)?;
    Ok((input, Expression::CJust(Box::new(expr))))
}

fn nothing_expression(input: &str) -> IResult<&str, Expression> {
    map(tag("Nothing"), |_| Expression::CNothing)(input)
}

fn isnothing_expression(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("isNothing")(input)?;
    let (input, _) = space0(input)?;

    let (input, expr) = delimited(
        tuple((char('('), space0)),
        expression,
        tuple((space0, char(')'))),
    )(input)?;

    Ok((input, Expression::IsNothing(Box::new(expr))))
}

fn iserror_expression(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("isError")(input)?;
    let (input, _) = space0(input)?;
    let (input, expr) = delimited(
        tuple((char('('), space0)),
        expression,
        tuple((space0, char(')'))),
    )(input)?;

    Ok((input, Expression::IsError(Box::new(expr))))
}

fn unwrap_expression(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("unwrap")(input)?;
    let (input, _) = space0(input)?;
    let (input, expr) = delimited(
        tuple((char('('), space0)),
        expression,
        tuple((space0, char(')'))),
    )(input)?;

    Ok((input, Expression::Unwrap(Box::new(expr))))
}

fn tryunwrap_expression(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("tryUnwrap")(input)?;
    let (input, _) = space0(input)?;
    let (input, expr) = delimited(
        tuple((char('('), space0)),
        expression,
        tuple((space0, char(')'))),
    )(input)?;

    Ok((input, Expression::Propagate(Box::new(expr))))
}

// Parse boolean operations
fn boolean_expression(input: &str) -> IResult<&str, Expression> {
    let (input, first) = boolean_term(input)?;
    let (input, rest) = many0(tuple((
        delimited(space0, alt((tag("and"), tag("or"))), space0),
        boolean_term,
    )))(input)?;

    Ok((
        input,
        rest.into_iter().fold(first, |acc, (op, val)| match op {
            "and" => Expression::And(Box::new(acc), Box::new(val)),
            "or" => Expression::Or(Box::new(acc), Box::new(val)),
            _ => unreachable!(),
        }),
    ))
}

fn boolean_term(input: &str) -> IResult<&str, Expression> {
    alt((
        map(preceded(tag("not "), boolean_factor), |expr| {
            Expression::Not(Box::new(expr))
        }),
        boolean_factor,
    ))(input)
}

fn boolean_factor(input: &str) -> IResult<&str, Expression> {
    alt((
        boolean,
        comparison_expression,
        unwrap_expression,
        tryunwrap_expression,
        iserror_expression,
        isnothing_expression,
        delimited(
            tuple((char('('), space0)),
            boolean_expression,
            tuple((space0, char(')'))),
        ),
    ))(input)
}

fn factor(input: &str) -> IResult<&str, Expression> {
    alt((
        delimited(
            tuple((char('('), space0)),
            arithmetic_expression,
            tuple((space0, char(')'))),
        ),
        function_call,
        ok_expression,
        err_expression,
        just_expression,
        nothing_expression,
        unwrap_expression,
        tryunwrap_expression,
        iserror_expression,
        isnothing_expression,
        real,
        integer,
        map(tuple((char('-'), space0, factor)), |(_, _, expr)| {
            Expression::Mul(Box::new(Expression::CInt(-1)), Box::new(expr))
        }),
        map(identifier, Expression::Var),
    ))(input)
}

//indented block parser
fn indented_block(input: &str) -> IResult<&str, Vec<Statement>> {
    let (input, _) = line_ending(input)?;
    let (input, statements) = separated_list1(
        line_ending,
        preceded(
            space1, // Require at least one space for indentation
            statement,
        ),
    )(input)?;
    Ok((input, statements))
}

fn if_statement(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("if")(input)?;
    let (input, _) = space1(input)?;
    let (input, condition) = alt((
        comparison_expression,
        boolean_expression,
        map(identifier, Expression::Var),
    ))(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char(':')(input)?;
    let (input, then_block) = indented_block(input)?;

    let (input, else_block) = opt(preceded(
        tuple((line_ending, space0, tag("else"), char(':'))),
        indented_block,
    ))(input)?;

    Ok((
        input,
        Statement::IfThenElse(
            Box::new(condition),
            Box::new(Statement::Block(then_block)),
            else_block.map(|stmts| Box::new(Statement::Block(stmts))),
        ),
    ))
}

// A 'for' statement parser.
// A basic 'for' statement in Python has the following
// syntax:
//
// > for <var> in <exp>:
// >  <stmt>
fn for_statement(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("for")(input)?;
    let (input, _) = space1(input)?;
    let (input, var) = identifier(input)?;
    let (input, _) = space1(input)?;
    let (input, _) = tag("in")(input)?;
    let (input, _) = space1(input)?;
    let (input, exp) = expression(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char(':')(input)?;
    let (input, block) = indented_block(input)?;
    Ok((
        input,
        Statement::For(var, Box::new(exp), Box::new(Statement::Block(block))),
    ))
}

fn declaration(input: &str) -> IResult<&str, Statement> {
    let (input, keyword) = alt((tag("var"), tag("val")))(input)?;
    let (input, _) = space1(input)?;
    let (input, name) = identifier(input)?;

    Ok((
        input,
        match keyword {
            "var" => Statement::VarDeclaration(name), // No Box needed
            "val" => Statement::ValDeclaration(name), // No Box needed
            _ => unreachable!(),
        },
    ))
}

// Parse assignment statements
fn assignment(input: &str) -> IResult<&str, Statement> {
    let (input, name) = identifier(input)?;
    let (input, _) = delimited(space0, char('='), space0)(input)?;
    let (input, expr) = expression(input)?;

    Ok((input, Statement::Assignment(name, Box::new(expr))))
}

fn parse_type(type_name: &str) -> Type {
    match type_name {
        "TInteger" => Type::TInteger,
        "TBool" => Type::TBool,
        "TReal" => Type::TReal,
        _ => Type::TInteger, // Default case
    }
}

// function definition parsing
fn function_def(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("def")(input)?;
    let (input, _) = space1(input)?;
    let (input, name) = identifier(input)?;
    let (input, _) = char('(')(input)?;
    let (input, params) = separated_list0(
        delimited(space0, char(','), space0),
        tuple((
            identifier,
            preceded(tuple((space0, char(':'), space0)), identifier),
        )),
    )(input)?;
    let (input, _) = char(')')(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = tag("->")(input)?;
    let (input, _) = space0(input)?;
    let (input, return_type) = identifier(input)?;
    let (input, _) = char(':')(input)?;
    let (input, body) = indented_block(input)?;

    Ok((
        input,
        Statement::FuncDef(Function {
            name: name.clone(),                   // Provide the name field
            kind: Some(parse_type(&return_type)), // Wrap in Some
            params: Some(
                params
                    .into_iter()
                    .map(|(name, type_name)| (name, parse_type(&type_name)))
                    .collect(),
            ),
            body: Some(Box::new(Statement::Block(body))), // Wrap in Some
        }),
    ))
}

//return statement parsing
fn return_statement(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("return")(input)?;
    let (input, _) = space1(input)?;
    let (input, expr) = expression(input)?;
    Ok((input, Statement::Return(Box::new(expr))))
}

// Parse multiple statements
pub fn parse_statements(input: &str) -> IResult<&str, Vec<Statement>> {
    let (input, _) = space0(input)?; // Handle initial whitespace
    let (input, statements) = separated_list1(
        many1(tuple((space0, line_ending, space0))), // Require at least one newline
        statement, // Use statement directly instead of limited alternatives
    )(input)?;
    let (input, _) = space0(input)?; // Handle trailing whitespace
    Ok((input, statements))
}

// function call parsing
fn function_call(input: &str) -> IResult<&str, Expression> {
    let (input, name) = identifier(input)?;
    let (input, _) = char('(')(input)?;
    let (input, args) = separated_list0(delimited(space0, char(','), space0), expression)(input)?;
    let (input, _) = char(')')(input)?;

    Ok((input, Expression::FuncCall(name, args)))
}

// Main parse function
pub fn parse(input: &str) -> IResult<&str, Vec<Statement>> {
    let (input, statements) = parse_statements(input)?;
    let (input, _) = many0(line_ending)(input)?; // Consume trailing newlines
    let (input, _) = space0(input)?; // Consume trailing whitespace
    Ok((input, statements))
}

fn adt_declaration(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("adt")(input)?;
    let (input, _) = space1(input)?;
    let (input, name) = identifier(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char('=')(input)?;
    let (input, _) = space0(input)?;
    let (input, constructors) = separated_list1(
        preceded(space0, char('|')),         // Match `|`, allowing leading spaces
        preceded(space0, value_constructor), // Consume extra spaces before each constructor
    )(input)?;

    Ok((input, Statement::ADTDeclaration(name, constructors)))
}

fn value_constructor(input: &str) -> IResult<&str, ValueConstructor> {
    let (input, name) = identifier(input)?;
    let (input, types) = many0(preceded(space1, type_annotation))(input)?;

    Ok((input, ValueConstructor { name, types }))
}

fn type_annotation(input: &str) -> IResult<&str, Type> {
    alt((
        map(tag("Int"), |_| Type::TInteger),
        map(tag("Bool"), |_| Type::TBool),
        map(tag("Real"), |_| Type::TReal),
        map(tag("String"), |_| Type::TString),
        map(tag("Any"), |_| Type::TAny),
    ))(input)
}

fn match_expression(input: &str) -> IResult<&str, Statement> {
    let (input, _) = multispace0(input)?; // Skip leading spaces & newlines
    let (input, _) = tag("match")(input)?; // Parse the "match" keyword
    let (input, _) = space1(input)?; // Require at least one space after "match"
    let (input, exp) = expression(input)?; // Parse the expression to match
    let (input, _) = multispace0(input)?; // Skip spaces & newlines
    let (input, _) = char('{')(input)?; // Parse the opening brace
    let (input, _) = multispace0(input)?; // Skip spaces & newlines

    // Parse the match cases
    let (input, cases) = separated_list0(
        tuple((multispace0, char(','), multispace0)), // Allow spaces/newlines before and after `,`
        match_case,                                   // Parse each match case
    )(input)?;

    let (input, _) = multispace0(input)?; // Skip spaces & newlines
    let (input, _) = char('}')(input)?; // Parse the closing brace

    Ok((input, Statement::Match(Box::new(exp), cases)))
}

fn match_case(input: &str) -> IResult<&str, (Expression, Box<Statement>)> {
    //println!("Parsing match case: {}", input); // Debug print
    let (input, _) = multispace0(input)?; // Skip spaces & newlines
                                          //println!("After skipping spaces: {}", input); // Debug print
    let (input, pattern) = pattern(input)?;
    //println!("Parsed pattern: {:?}", pattern); // Debug print
    let (input, _) = space0(input)?; // Skip optional spaces
                                     //println!("After skipping spaces before =>: {}", input); // Debug print
    let (input, _) = tag("=>")(input)?; // Parse the "=>" operator
                                        //println!("After parsing =>: {}", input); // Debug print
    let (input, _) = space0(input)?; // Skip optional spaces
                                     //println!("After skipping spaces after =>: {}", input); // Debug print
    let (input, stmt) = statement(input)?;
    //println!("Parsed statement: {:?}", stmt); // Debug print

    Ok((input, (pattern, Box::new(stmt))))
}
fn pattern(input: &str) -> IResult<&str, Expression> {
    alt((
        adt_pattern,                      // Handle ADT patterns first (e.g., "Circle r")
        map(identifier, Expression::Var), // Fallback to variables
    ))(input)
}

fn arg_pattern(input: &str) -> IResult<&str, Expression> {
    map(identifier, Expression::Var)(input) // Only parse variables
}

fn adt_pattern(input: &str) -> IResult<&str, Expression> {
    let (input, adt_name) = identifier(input)?; // Parse the ADT name
    let (input, _) = space0(input)?; // Skip optional spaces
    let (input, constructor_name) = identifier(input)?; // Parse the constructor name
    let (input, args) = many1(preceded(space1, arg_pattern))(input)?; // Parse the arguments

    Ok((
        input,
        Expression::ADTConstructor(
            adt_name,
            constructor_name,
            args.into_iter().map(Box::new).collect(),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*; // Import everything from parent module
    use crate::ir::ast::{Expression, Statement}; // Import AST types
    #[test]
    fn test_simple_assignment() {
        let input = "x = 42";
        let (rest, stmt) = assignment(input).unwrap();
        assert_eq!(rest, "");
        match stmt {
            Statement::Assignment(name, expr) => {
                // Added _type
                assert_eq!(name, "x");
                match *expr {
                    Expression::CInt(val) => assert_eq!(val, 42),
                    _ => panic!("Expected CInt"),
                }
            }
            _ => panic!("Expected Assignment"),
        }
    }

    #[test]
    fn test_complete_program() {
        let input = "x = 10\nif x > 5:\n    y = 1\nelse:\n    y = 2";
        let (rest, stmts) = parse(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(stmts.len(), 2); // Assignment and IfThenElse
    }

    #[test]
    fn test_complex_expression() {
        let input = "x = (2 * 3) + (10 - 4)";
        let (rest, stmts) = parse(input).unwrap();
        assert_eq!(rest, "");

        match &stmts[0] {
            Statement::Assignment(name, expr) => {
                // Added _type
                assert_eq!(name, "x");
                match **expr {
                    Expression::Add(_, _) => (),
                    _ => panic!("Expected Add expression"),
                }
            }
            _ => panic!("Expected Assignment"),
        }
    }

    #[test]
    fn test_multiline_with_if() {
        let input = "x = 10\nif x > 5:\n    y = 1\nelse:\n    y = 2";
        let (rest, stmts) = parse(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(stmts.len(), 2); // Should have assignment and if-statement

        // Verify first statement is assignment
        match &stmts[0] {
            Statement::Assignment(name, expr) => {
                // Added _type
                assert_eq!(name, "x");
                assert!(matches!(**expr, Expression::CInt(10)));
            }
            _ => panic!("Expected Assignment"),
        }

        // Verify second statement is if-else
        match &stmts[1] {
            Statement::IfThenElse(condition, then_block, else_block) => {
                // Check condition - using GT instead of Comparison
                assert!(matches!(**condition, Expression::GT(_, _)));

                // Check then block
                match **then_block {
                    Statement::Block(ref stmts) => {
                        assert_eq!(stmts.len(), 1);
                        match &stmts[0] {
                            Statement::Assignment(name, expr) => {
                                assert_eq!(name, "y");
                                assert!(matches!(**expr, Expression::CInt(1)));
                            }
                            _ => panic!("Expected Assignment in then block"),
                        }
                    }
                    _ => panic!("Expected Block"),
                }

                // Check else block
                match else_block {
                    Some(else_stmt) => match **else_stmt {
                        Statement::Block(ref stmts) => {
                            assert_eq!(stmts.len(), 1);
                            match &stmts[0] {
                                Statement::Assignment(name, expr) => {
                                    assert_eq!(name, "y");
                                    assert!(matches!(**expr, Expression::CInt(2)));
                                }
                                _ => panic!("Expected Assignment in else block"),
                            }
                        }
                        _ => panic!("Expected Block"),
                    },
                    None => panic!("Expected Some else block"),
                }
            }
            _ => panic!("Expected IfThenElse"),
        }
    }

    #[test]
    fn test_if_else_block() {
        let input = "if x > 0:\n    y = 1\nelse:\n    y = 2";
        let (rest, stmt) = if_statement(input).unwrap();
        assert_eq!(rest, "");

        match stmt {
            Statement::IfThenElse(condition, then_block, else_block) => {
                // Check condition
                assert!(matches!(*condition, Expression::GT(_, _)));

                // Check then block
                match *then_block {
                    Statement::Block(ref stmts) => {
                        assert_eq!(stmts.len(), 1);
                        match &stmts[0] {
                            Statement::Assignment(name, expr) => {
                                assert_eq!(name, "y");
                                assert!(matches!(**expr, Expression::CInt(1)));
                            }
                            _ => panic!("Expected Assignment in then block"),
                        }
                    }
                    _ => panic!("Expected Block"),
                }

                // Check else block
                match else_block {
                    Some(else_stmt) => match *else_stmt {
                        Statement::Block(ref stmts) => {
                            assert_eq!(stmts.len(), 1);
                            match &stmts[0] {
                                Statement::Assignment(name, expr) => {
                                    assert_eq!(name, "y");
                                    assert!(matches!(**expr, Expression::CInt(2)));
                                }
                                _ => panic!("Expected Assignment in else block"),
                            }
                        }
                        _ => panic!("Expected Block"),
                    },
                    None => panic!("Expected Some else block"),
                }
            }
            _ => panic!("Expected IfThenElse"),
        }
    }

    #[test]
    fn test_if_else_statement() {
        let input = "if x > 0:\n    y = 1\nelse:\n    y = 2";
        let (rest, stmt) = if_statement(input).unwrap();
        assert_eq!(rest, "");

        match stmt {
            Statement::IfThenElse(condition, then_block, else_block) => {
                // Check condition
                assert!(matches!(
                    *condition,
                    Expression::GT(_box_ref @ _, _box_ref2 @ _)
                ));

                // Check then block
                match *then_block {
                    Statement::Block(ref stmts) => {
                        assert_eq!(stmts.len(), 1);
                        match &stmts[0] {
                            Statement::Assignment(name, expr) => {
                                assert_eq!(name, "y");
                                assert!(matches!(**expr, Expression::CInt(1)));
                            }
                            _ => panic!("Expected Assignment"),
                        }
                    }
                    _ => panic!("Expected Block"),
                }

                // Check else block
                match else_block {
                    Some(else_stmt) => match *else_stmt {
                        Statement::Block(ref stmts) => {
                            assert_eq!(stmts.len(), 1);
                            match &stmts[0] {
                                Statement::Assignment(name, expr) => {
                                    assert_eq!(name, "y");
                                    assert!(matches!(**expr, Expression::CInt(2)));
                                }
                                _ => panic!("Expected Assignment"),
                            }
                        }
                        _ => panic!("Expected Block"),
                    },
                    None => panic!("Expected Some else block"),
                }
            }
            _ => panic!("Expected IfThenElse"),
        }
    }

    #[test]
    fn test_for_statement() {
        let input = "for x in range:\n   x = x+1";
        let (rest, stmt) = statement(input).unwrap();
        let expected = Statement::For(
            "x".to_string(),
            Box::new(Expression::Var("range".to_string())),
            Box::new(Statement::Block(
                [Statement::Assignment(
                    "x".to_string(),
                    Box::new(Expression::Add(
                        Box::new(Expression::Var("x".to_string())),
                        Box::new(Expression::CInt(1)),
                    )),
                )]
                .to_vec(),
            )),
        );
        assert_eq!(rest, "");
        assert_eq!(stmt, expected)
    }

    #[test]
    fn test_multiline_parse() {
        let input = "x = 42\ny = 10";
        let (rest, stmts) = parse(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(stmts.len(), 2);

        match &stmts[0] {
            Statement::Assignment(name, expr) => {
                assert_eq!(&**name, "x");
                match **expr {
                    Expression::CInt(42) => (),
                    _ => panic!("Expected CInt(42)"),
                }
            }
            _ => panic!("Expected Assignment"),
        }

        match &stmts[1] {
            Statement::Assignment(name, expr) => {
                assert_eq!(&**name, "y");
                match **expr {
                    Expression::CInt(10) => (),
                    _ => panic!("Expected CInt(10)"),
                }
            }
            _ => panic!("Expected Assignment"),
        }
    }

    #[test]
    fn test_whitespace_handling() {
        let input = "   x    =    42   \n   y   =   10   ";
        let (rest, stmts) = parse(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_function_definition() {
        let input = r#"def add(x: TInteger, y: TInteger) -> TInteger:
        return x + y"#;
        let (rest, stmt) = function_def(input).unwrap();
        assert_eq!(rest, "");
        match stmt {
            Statement::FuncDef(func) => {
                assert_eq!(func.name, "add");
                assert_eq!(func.kind, Some(Type::TInteger));
                match func.params {
                    Some(params) => {
                        assert_eq!(params.len(), 2);
                        assert_eq!(params[0].0, "x");
                        assert_eq!(params[1].0, "y");
                    }
                    None => panic!("Expected Some params"),
                }
                assert_eq!(
                    func.body,
                    Some(Box::new(Statement::Block(vec![Statement::Return(
                        Box::new(Expression::Add(
                            Box::new(Expression::Var("x".to_string())),
                            Box::new(Expression::Var("y".to_string()))
                        ))
                    )])))
                );
            }
            _ => panic!("Expected FuncDef"),
        }
    }

    #[test]
    fn test_function_call() {
        let input = "result = add(5, 3)";
        let (rest, stmt) = assignment(input).unwrap();
        assert_eq!(rest, "");
        match stmt {
            Statement::Assignment(name, expr) => {
                assert_eq!(name, "result");
                match *expr {
                    Expression::FuncCall(func_name, args) => {
                        assert_eq!(func_name, "add");
                        assert_eq!(args.len(), 2);
                    }
                    _ => panic!("Expected FuncCall"),
                }
            }
            _ => panic!("Expected Assignment"),
        }
    }

    #[test]
    fn test_basic_arithmetic_left_recursion() {
        let cases = vec![
            (
                "1 + 2",
                Expression::Add(Box::new(Expression::CInt(1)), Box::new(Expression::CInt(2))),
            ),
            (
                "3 * 4",
                Expression::Mul(Box::new(Expression::CInt(3)), Box::new(Expression::CInt(4))),
            ),
        ];

        for (input, expected) in cases {
            let (rest, result) = arithmetic_expression(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_operator_precedence() {
        let input = "2 + 3 * 4";
        let expected = Expression::Add(
            Box::new(Expression::CInt(2)),
            Box::new(Expression::Mul(
                Box::new(Expression::CInt(3)),
                Box::new(Expression::CInt(4)),
            )),
        );

        let (rest, result) = arithmetic_expression(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_left_associativity() {
        let input = "1 - 2 - 3"; // Should parse as (1-2)-3, not 1-(2-3)
        let expected = Expression::Sub(
            Box::new(Expression::Sub(
                Box::new(Expression::CInt(1)),
                Box::new(Expression::CInt(2)),
            )),
            Box::new(Expression::CInt(3)),
        );

        let (rest, result) = arithmetic_expression(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_nested_expressions() {
        let input = "(1 + 2) * (3 + 4)";
        let expected = Expression::Mul(
            Box::new(Expression::Add(
                Box::new(Expression::CInt(1)),
                Box::new(Expression::CInt(2)),
            )),
            Box::new(Expression::Add(
                Box::new(Expression::CInt(3)),
                Box::new(Expression::CInt(4)),
            )),
        );

        let (rest, result) = arithmetic_expression(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_complex_expression_2() {
        let input = "1 + 2 * 3 + 4 * 5";
        let expected = Expression::Add(
            Box::new(Expression::Add(
                Box::new(Expression::CInt(1)),
                Box::new(Expression::Mul(
                    Box::new(Expression::CInt(2)),
                    Box::new(Expression::CInt(3)),
                )),
            )),
            Box::new(Expression::Mul(
                Box::new(Expression::CInt(4)),
                Box::new(Expression::CInt(5)),
            )),
        );

        let (rest, result) = arithmetic_expression(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_negative_numbers_with_operations() {
        let cases = vec![
            (
                "-1 + 2",
                Expression::Add(
                    Box::new(Expression::CInt(-1)),
                    Box::new(Expression::CInt(2)),
                ),
            ),
            (
                "3 * -4",
                Expression::Mul(
                    Box::new(Expression::CInt(3)),
                    Box::new(Expression::CInt(-4)),
                ),
            ),
        ];

        for (input, expected) in cases {
            let (rest, result) = arithmetic_expression(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_boolean_literals() {
        let cases = vec![("True", Expression::CTrue), ("False", Expression::CFalse)];

        for (input, expected) in cases {
            let (rest, result) = boolean(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_real_numbers() {
        let cases = vec![
            ("3.14", Expression::CReal(3.14)),
            ("-2.5", Expression::CReal(-2.5)),
            ("0.0", Expression::CReal(0.0)),
        ];

        for (input, expected) in cases {
            let (rest, result) = real(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_string_literals() {
        let cases = vec![
            ("\"hello\"", Expression::CString("hello".to_string())),
            ("\"123\"", Expression::CString("123".to_string())),
            ("\"\"", Expression::CString("".to_string())),
        ];

        for (input, expected) in cases {
            let (rest, result) = string(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_boolean_operations() {
        let cases = vec![
            (
                "True and False",
                Expression::And(Box::new(Expression::CTrue), Box::new(Expression::CFalse)),
            ),
            (
                "True or False",
                Expression::Or(Box::new(Expression::CTrue), Box::new(Expression::CFalse)),
            ),
            ("not True", Expression::Not(Box::new(Expression::CTrue))),
        ];

        for (input, expected) in cases {
            let (rest, result) = boolean_expression(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_complex_boolean_expressions() {
        let input = "not (True and False) or True";
        let expected = Expression::Or(
            Box::new(Expression::Not(Box::new(Expression::And(
                Box::new(Expression::CTrue),
                Box::new(Expression::CFalse),
            )))),
            Box::new(Expression::CTrue),
        );

        let (rest, result) = boolean_expression(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_iserror_err_expression() {
        let input = "isError (Err (1))";
        let (rest, result) = iserror_expression(input).unwrap();
        let expected =
            Expression::IsError(Box::new(Expression::CErr(Box::new(Expression::CInt(1)))));
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_iserror_ok_expression() {
        let input = "isError (Ok (2))";
        let (rest, result) = iserror_expression(input).unwrap();
        let expected =
            Expression::IsError(Box::new(Expression::COk(Box::new(Expression::CInt(2)))));
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_iserror_real() {
        let input = "isError (3.14)";
        let (rest, result) = iserror_expression(input).unwrap();
        let expected = Expression::IsError(Box::new(Expression::CReal(3.14)));
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_isnothing_nothing_expression() {
        let input = "isNothing(Nothing)";
        let (rest, result) = isnothing_expression(input).unwrap();
        let expected = Expression::IsNothing(Box::new(Expression::CNothing));
        assert_eq!(rest, "");
        assert_eq!(result, expected);
        //Necessita da implementação de definição de Nothing.
    }

    #[test]
    fn test_eval_isnothing_just_expression() {
        let input = "isNothing (Just (2))";
        let (rest, result) = isnothing_expression(input).unwrap();
        let expected =
            Expression::IsNothing(Box::new(Expression::CJust(Box::new(Expression::CInt(2)))));
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_isnothing_real() {
        let input = "isNothing (4.20)";
        let (rest, result) = isnothing_expression(input).unwrap();
        let expected = Expression::IsNothing(Box::new(Expression::CReal(4.20)));
        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_ok_creation() {
        let cases = vec![
            ("Ok(1)", Expression::COk(Box::new(Expression::CInt(1)))),
            ("Ok(0.5)", Expression::COk(Box::new(Expression::CReal(0.5)))),
            ("Err(False)", Expression::CErr(Box::new(Expression::CFalse))),
        ];

        for (input, expected) in cases {
            let (rest, result) = expression(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_try_unwrap_expression() {
        let input = "tryUnwrap(Ok(42))";
        let expected =
            Expression::Propagate(Box::new(Expression::COk(Box::new(Expression::CInt(42)))));

        let (remaining, parsed) = tryunwrap_expression(input).expect("Parsing failed");

        assert_eq!(parsed, expected);
        assert!(
            remaining.is_empty(),
            "Remaining input should be empty but got: {}",
            remaining
        );
    }

    #[test]
    fn test_unwrap_parsing() {
        let cases = vec![
            (
                "unwrap(Ok(2))",
                Expression::Unwrap(Box::new(Expression::COk(Box::new(Expression::CInt(2))))),
            ),
            (
                "unwrap(Ok(2.5))",
                Expression::Unwrap(Box::new(Expression::COk(Box::new(Expression::CReal(2.5))))),
            ),
            (
                "unwrap(3)",
                Expression::Unwrap(Box::new(Expression::CInt(3))),
            ),
            (
                "unwrap(3.5)",
                Expression::Unwrap(Box::new(Expression::CReal(3.5))),
            ),
        ];

        for (input, expected) in cases {
            let (rest, result) = unwrap_expression(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_propagation_parsing() {
        let cases = vec![
            (
                "tryUnwrap(Ok(2))",
                Expression::Propagate(Box::new(Expression::COk(Box::new(Expression::CInt(2))))),
            ),
            (
                "tryUnwrap(tryUnwrap(x))",
                Expression::Propagate(Box::new(Expression::Propagate(Box::new(Expression::Var(
                    String::from("x"),
                ))))),
            ),
            (
                "tryUnwrap(Ok(10.1 + 1.2))",
                Expression::Propagate(Box::new(Expression::COk(Box::new(Expression::Add(
                    Box::new(Expression::CReal(10.1)),
                    Box::new(Expression::CReal(1.2)),
                ))))),
            ),
            /*(
                "tryUnwrap(Ok(1)) / tryUnwrap(Just(2))",
                Expression::Div(
                    Box::new(Expression::Propagate(Box::new(Expression::COk(Box::new(
                        Expression::CInt(1),
                    ))))),
                    Box::new(Expression::Propagate(Box::new(Expression::CJust(
                        Box::new(Expression::CInt(2)),
                    )))),
                ),
            ),*/
            (
                "tryUnwrap(Ok(True)) and tryUnwrap(Ok(False))",
                Expression::And(
                    Box::new(Expression::Propagate(Box::new(Expression::COk(Box::new(
                        Expression::CTrue,
                    ))))),
                    Box::new(Expression::Propagate(Box::new(Expression::COk(Box::new(
                        Expression::CFalse,
                    ))))),
                ),
            ),
            (
                "tryUnwrap(tryUnwrap(Ok(True or False)))",
                Expression::Propagate(Box::new(Expression::Propagate(Box::new(Expression::COk(
                    Box::new(Expression::Or(
                        Box::new(Expression::CTrue),
                        Box::new(Expression::CFalse),
                    )),
                ))))),
            ),
            (
                "tryUnwrap(Just(not False))",
                Expression::Propagate(Box::new(Expression::CJust(Box::new(Expression::Not(
                    Box::new(Expression::CFalse),
                ))))),
            ),
        ];

        for (input, expected) in cases {
            let (rest, result) = expression(input).unwrap();
            assert_eq!(rest, "");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_propagation_parsing_statements() {
        let input = "x = Ok(True)\nif unwrap(x):\n  y = 1\nif tryUnwrap(x):\n  y = 1\n";

        let (rest, result) = parse(input).unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            result,
            [
                Statement::Assignment(
                    String::from("x"),
                    Box::new(Expression::COk(Box::new(Expression::CTrue)))
                ),
                Statement::IfThenElse(
                    Box::new(Expression::Unwrap(Box::new(Expression::Var(String::from(
                        "x"
                    ))))),
                    Box::new(Statement::Block(vec![Statement::Assignment(
                        String::from("y"),
                        Box::new(Expression::CInt(1))
                    )])),
                    None
                ),
                Statement::IfThenElse(
                    Box::new(Expression::Propagate(Box::new(Expression::Var(
                        String::from("x")
                    )))),
                    Box::new(Statement::Block(vec![Statement::Assignment(
                        String::from("y"),
                        Box::new(Expression::CInt(1))
                    )])),
                    None
                )
            ]
        );
    }

    #[test]
    fn test_eval_just_integer() {
        let input = "Just (42)";
        let (rest, result) = just_expression(input).unwrap();
        let expected = Expression::CJust(Box::new(Expression::CInt(42)));

        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_just_real() {
        let input = "Just (3.14)";
        let (rest, result) = just_expression(input).unwrap();
        let expected = Expression::CJust(Box::new(Expression::CReal(3.14)));

        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_just_expression() {
        let input = "Just (1 + 2)";
        let (rest, result) = just_expression(input).unwrap();
        let expected = Expression::CJust(Box::new(Expression::Add(
            Box::new(Expression::CInt(1)),
            Box::new(Expression::CInt(2)),
        )));

        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_nothing() {
        let input = "Nothing";
        let (rest, result) = nothing_expression(input).unwrap();
        let expected = Expression::CNothing;

        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eval_isnothing_nothing() {
        let input = "isNothing (Nothing)";
        let (rest, result) = isnothing_expression(input).unwrap();
        let expected = Expression::IsNothing(Box::new(Expression::CNothing));

        assert_eq!(rest, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_create_function_with_keyword_if() {
        let input = "def if(x: TInteger) -> TInteger:\n    return x";
        let result = function_def(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_create_function_with_keyword_while() {
        let input = "def while(x: TInteger) -> TInteger:\n    return x";
        let result = function_def(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_create_function_with_keyword_ok() {
        let input = "def Ok(x: TInteger) -> TInteger:\n    return x";
        let result = function_def(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_var_declaration_with_keyword_if() {
        let input = "if = 10";
        let result = assignment(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_var_declaration_with_keyword_while() {
        let input = "while = 10";
        let result = assignment(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_var_declaration_with_keyword_ok() {
        let input = "Ok = 10";
        let result = assignment(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_adt_pattern() {
        // Test case 1: Circle with one argument
        let input = "Shape Circle r";
        let result = adt_pattern(input);
        assert!(result.is_ok());
        let (remaining_input, parsed_expr) = result.unwrap();
        assert_eq!(remaining_input, ""); // Ensure the entire input is consumed
        assert_eq!(
            parsed_expr,
            Expression::ADTConstructor(
                "Shape".to_string(),                              // ADT name
                "Circle".to_string(),                             // Constructor name
                vec![Box::new(Expression::Var("r".to_string()))]  // Argument
            )
        );

        println!("Passou");

        // Test case 2: Rectangle with two arguments
        let input = "Shape Rectangle w h";
        let result = adt_pattern(input);
        assert!(result.is_ok());
        let (remaining_input, parsed_expr) = result.unwrap();
        assert_eq!(remaining_input, ""); // Ensure the entire input is consumed
        assert_eq!(
            parsed_expr,
            Expression::ADTConstructor(
                "Shape".to_string(),     // ADT name
                "Rectangle".to_string(), // Constructor name
                vec![
                    Box::new(Expression::Var("w".to_string())), // First argument
                    Box::new(Expression::Var("h".to_string()))  // Second argument
                ]
            )
        );

        // Test case 3: Triangle with three arguments
        let input = "Shape Triangle b h s";
        let result = adt_pattern(input);
        assert!(result.is_ok());
        let (remaining_input, parsed_expr) = result.unwrap();
        assert_eq!(remaining_input, ""); // Ensure the entire input is consumed
        assert_eq!(
            parsed_expr,
            Expression::ADTConstructor(
                "Shape".to_string(),    // ADT name
                "Triangle".to_string(), // Constructor name
                vec![
                    Box::new(Expression::Var("b".to_string())), // First argument
                    Box::new(Expression::Var("h".to_string())), // Second argument
                    Box::new(Expression::Var("s".to_string()))  // Third argument
                ]
            )
        );

        // Test case 4: Invalid input (missing argument)
        let input = "Shape Circle";
        let result = adt_pattern(input);
        assert!(result.is_err()); // Expect an error because the argument is missing
    }

    #[test]
    fn parser_test_adt_and_pattern_matching2() {
        // Define the ADT for geometric shapes
        let adt_input = "adt FG = Circle Bool | Rectangle Bool Bool | Triangle Bool Bool Bool";
        println!("Parsing ADT: {}", adt_input);
        let adt_result = adt_declaration(adt_input);
        println!("ADT Result: {:?}", adt_result);
        assert!(adt_result.is_ok());

        // Define the match expression
        let match_input = "
            match shape {
                FG Circle r => return 3.14 * r * r,
                FG Rectangle w h => return w * h,
                FG Triangle b h s => return 0.5 * b * h
            }
        ";
        println!("Parsing Match Expression: {}", match_input);
        let match_result = match_expression(match_input);
        println!("Match Result: {:?}", match_result);
        assert!(match_result.is_ok());

        // Verify the parsed ADT
        let (_, adt) = adt_result.unwrap();
        println!("Parsed ADT: {:?}", adt);
        assert_eq!(
            adt,
            Statement::ADTDeclaration(
                "FG".to_string(),
                vec![
                    ValueConstructor {
                        name: "Circle".to_string(),
                        types: vec![Type::TBool],
                    },
                    ValueConstructor {
                        name: "Rectangle".to_string(),
                        types: vec![Type::TBool, Type::TBool],
                    },
                    ValueConstructor {
                        name: "Triangle".to_string(),
                        types: vec![Type::TBool, Type::TBool, Type::TBool],
                    },
                ]
            )
        );

        // Verify the parsed match expression
        let (_, match_stmt) = match_result.unwrap();
        println!("Parsed Match Statement: {:?}", match_stmt);
        assert_eq!(
            match_stmt,
            Statement::Match(
                Box::new(Expression::Var("shape".to_string())),
                vec![
                    (
                        Expression::ADTConstructor(
                            "FG".to_string(),
                            "Circle".to_string(),
                            vec![Box::new(Expression::Var("r".to_string()))]
                        ),
                        Box::new(Statement::Return(Box::new(Expression::Mul(
                            Box::new(Expression::Mul(
                                Box::new(Expression::CReal(3.14)),
                                Box::new(Expression::Var("r".to_string()))
                            )),
                            Box::new(Expression::Var("r".to_string()))
                        )))),
                    ),
                    (
                        Expression::ADTConstructor(
                            "FG".to_string(),
                            "Rectangle".to_string(),
                            vec![
                                Box::new(Expression::Var("w".to_string())),
                                Box::new(Expression::Var("h".to_string()))
                            ]
                        ),
                        Box::new(Statement::Return(Box::new(Expression::Mul(
                            Box::new(Expression::Var("w".to_string())),
                            Box::new(Expression::Var("h".to_string()))
                        )))),
                    ),
                    (
                        Expression::ADTConstructor(
                            "FG".to_string(),
                            "Triangle".to_string(),
                            vec![
                                Box::new(Expression::Var("b".to_string())),
                                Box::new(Expression::Var("h".to_string())),
                                Box::new(Expression::Var("s".to_string()))
                            ]
                        ),
                        Box::new(Statement::Return(Box::new(Expression::Mul(
                            Box::new(Expression::Mul(
                                Box::new(Expression::CReal(0.5)),
                                Box::new(Expression::Var("b".to_string())),
                            )),
                            Box::new(Expression::Var("h".to_string()))
                        )))),
                    ),
                ]
            )
        );
    }
}
