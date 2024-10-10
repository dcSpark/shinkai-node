pub fn convert_rowcol_to_excel_index(row: usize, col: usize) -> String {
    let mut result = String::new();
    let mut col = col + 1;
    let row = row + 1;

    while col > 0 {
        let rem = (col - 1) % 26;
        result.push((rem as u8 + b'A') as char);
        col = (col - rem) / 26;
    }

    format!("{}{}", result.chars().rev().collect::<String>(), &row.to_string())
}

// Joins cells to a string and combines andjacent cells into ranges like A1:A3
pub fn combine_cells(cells: Vec<String>) -> String {
    let mut sorted_cells: Vec<_> = cells.iter().collect();
    sorted_cells.sort();
    let mut result = String::new();
    let mut range_start = sorted_cells[0];
    let mut range_end = sorted_cells[0];

    for i in 1..sorted_cells.len() {
        let current = sorted_cells[i];
        if is_adjacent(range_end, current) {
            range_end = current;
        } else {
            if range_start == range_end {
                result.push_str(range_start);
            } else {
                result.push_str(&format!("{}:{}", range_start, range_end));
            }
            result.push(',');
            range_start = current;
            range_end = current;
        }
    }

    if range_start == range_end {
        result.push_str(range_start);
    } else {
        result.push_str(&format!("{}:{}", range_start, range_end));
    }

    result
}

fn is_adjacent(cell1: &str, cell2: &str) -> bool {
    let (col1, row1) = parse_cell(cell1);
    let (col2, row2) = parse_cell(cell2);
    (col1 == col2 && row2 == row1 + 1) || (row1 == row2 && col2 as u8 == col1 as u8 + 1)
}

fn parse_cell(cell: &str) -> (char, u32) {
    let mut chars = cell.chars();
    let col = chars.next().unwrap();
    let row: u32 = chars.collect::<String>().parse().unwrap();
    (col, row)
}
