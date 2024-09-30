pub const STAGE_1_PROMPT: &str = r#"
INSTRUCTION:
Given an input that is a string denoting data of cells in a table.
The input table contains many tuples, describing the cells with content in the spreadsheet.
Each tuple consists of two elements separated by a ’|’: the cell content and the cell address/region, like (Year|A1), ( |A1) or (IntNum|A1:B3).
The content in some cells such as ’#,##0’/’d-mmm-yy’/’H:mm:ss’,etc., represents the CELL DATA FORMATS of Excel.
The content in some cells such as ’IntNum’/’DateData’/’EmailData’,etc., represents a category of data with the same format and similar semantics.
For example, ’IntNum’ represents integer type data, and ’ScientificNum’ represents scientific notation type data.’A1:B3’ represents a region in a spreadsheet,
from the first row to the third row and from column A to column B. Some cells with empty content in the spreadsheet are not entered.
How many tables are there in the spreadsheet? Below is a question about one certain table in this spreadsheet.
I need you to determine in which table the answer to the following question can be found, and return the RANGE of the ONE table you choose, LIKE [’range’: ’A1:F9’]. DON’T ADD OTHER WORDS OR EXPLANATION.

INPUT: {table_input}
"#;

pub const STAGE_2_PROMPT: &str = r#"
INSTRUCTION:
Given an input that is a string denoting data of cells in a table and a question about this table.
The answer to the question can be found in the table.
The input table includes many pairs, and each pair consists of a cell address and the text in that cell with a ',' in between, like 'A1,Year'.
Cells are separated by '|' like 'A1,Year|A2,Profit'.
The text can be empty so the cell data is like 'A1, |A2,Profit'.
The cells are organized in row-major order.
The answer to the input question is contained in the input table and can be represented by cell address.
I need you to find the cell address of the answer in the given table based on the given question description, and return the cell ADDRESS of the answer like '[B3]' or '[SUM(A2:A10)]'.
DON'T ADD ANY OTHER WORDS.
INPUT: {table_input}
"#;
