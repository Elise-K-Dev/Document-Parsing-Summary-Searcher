use std::collections::HashMap;
use std::error::Error;
use std::fs;

pub fn read_csv_records(path: &str) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let rows = parse_csv(&text)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let mut header = rows[0].clone();
    if let Some(first) = header.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }

    let mut records = Vec::new();
    for row in rows.iter().skip(1) {
        if row.len() != header.len() {
            continue;
        }
        let mut map = HashMap::new();
        for (key, value) in header.iter().zip(row.iter()) {
            map.insert(key.clone(), value.clone());
        }
        records.push(map);
    }
    Ok(records)
}

fn parse_csv(input: &str) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            match ch {
                '"' => {
                    if matches!(chars.peek(), Some('"')) {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                _ => field.push(ch),
            }
        } else {
            match ch {
                '"' => in_quotes = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                '\r' => {
                    if matches!(chars.peek(), Some('\n')) {
                        chars.next();
                    }
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(ch),
            }
        }
    }

    if in_quotes {
        return Err("unterminated quoted field".into());
    }

    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }

    rows.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    Ok(rows)
}
