use regex::Regex;
use shinkai_message_primitives::schemas::sheet::CellId;

pub struct CellNameConverter;

impl CellNameConverter {
    pub fn column_name_to_index(name: &str) -> usize {
        let mut index = 0;
        for (i, c) in name.chars().rev().enumerate() {
            index += (c as usize - 'A' as usize + 1) * 26_usize.pow(i as u32);
        }
        index - 1
    }

    pub fn column_index_to_name(index: usize) -> String {
        let mut index = index + 1;
        let mut name = String::new();
        while index > 0 {
            let rem = (index - 1) % 26;
            name.insert(0, (rem as u8 + b'A') as char);
            index = (index - rem - 1) / 26;
        }
        name
    }

    pub fn cell_name_to_indices(name: &str) -> (usize, usize) {
        eprintln!("cell_name_to_indices: {}", name);
        let re = Regex::new(r"([A-Z]+)(\d+)").unwrap();
        let caps = re.captures(name).unwrap();
        let col_name = &caps[1];
        let row_index: usize = caps[2].parse().unwrap();
        (row_index - 1, Self::column_name_to_index(col_name))
    }

    pub fn cell_indices_to_name(row: usize, col: usize) -> String {
        format!("{}{}", Self::column_index_to_name(col), row + 1)
    }

    pub fn cell_id_to_indices(cell_id: &CellId) -> (usize, usize) {
        let parts: Vec<&str> = cell_id.0.split(':').collect();
        (parts[0].parse().unwrap(), parts[1].parse().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_name_conversion() {
        assert_eq!(CellNameConverter::column_name_to_index("A"), 0);
        assert_eq!(CellNameConverter::column_name_to_index("Z"), 25);
        assert_eq!(CellNameConverter::column_name_to_index("AA"), 26);
        assert_eq!(CellNameConverter::column_name_to_index("AB"), 27);
        assert_eq!(CellNameConverter::column_index_to_name(0), "A");
        assert_eq!(CellNameConverter::column_index_to_name(25), "Z");
        assert_eq!(CellNameConverter::column_index_to_name(26), "AA");
        assert_eq!(CellNameConverter::column_index_to_name(27), "AB");
    }

    #[test]
    fn test_cell_name_conversion() {
        assert_eq!(CellNameConverter::cell_name_to_indices("A1"), (0, 0));
        assert_eq!(CellNameConverter::cell_name_to_indices("B2"), (1, 1));
        assert_eq!(CellNameConverter::cell_name_to_indices("AA10"), (9, 26));
        assert_eq!(CellNameConverter::cell_indices_to_name(0, 0), "A1");
        assert_eq!(CellNameConverter::cell_indices_to_name(1, 1), "B2");
        assert_eq!(CellNameConverter::cell_indices_to_name(9, 26), "AA10");
    }

    #[test]
    fn test_cell_id_to_indices() {
        let cell_id = CellId("0:1".to_string());
        assert_eq!(CellNameConverter::cell_id_to_indices(&cell_id), (0, 1));
    }
}
