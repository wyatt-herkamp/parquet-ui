use crate::wrangle::session::TABLE_NAME;

#[derive(Debug, Clone, Default)]
pub struct Pipeline {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone)]
pub enum Step {
    Sort {
        column: String,
        descending: bool,
        nulls_first: bool,
    },
    Filter {
        predicate: String,
    },
    DropColumns {
        columns: Vec<String>,
    },
    Rename {
        from: String,
        to: String,
    },
    Cast {
        column: String,
        target_type: String,
    },
    FillNa {
        column: String,
        value: String,
    },
    NullIf {
        column: String,
        value: String,
    },
    DropNa {
        columns: Vec<String>,
    },
    FindReplace {
        column: String,
        pattern: String,
        replacement: String,
        regex: bool,
    },
    Lowercase {
        column: String,
    },
    Uppercase {
        column: String,
    },
    Strip {
        column: String,
    },
    TextLength {
        column: String,
        new_column: String,
    },
    Round {
        column: String,
        decimals: i32,
    },
    Floor {
        column: String,
    },
    Ceiling {
        column: String,
    },
    DropDuplicates {
        columns: Vec<String>,
    },
    GroupByAggregate {
        keys: Vec<String>,
        aggregations: Vec<(String, AggFn, String)>,
    },
    FormulaColumn {
        new_column: String,
        expression: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum AggFn {
    Count,
    CountDistinct,
    Sum,
    Avg,
    Min,
    Max,
}

impl AggFn {
    pub fn sql(self) -> &'static str {
        match self {
            AggFn::Count => "COUNT",
            AggFn::CountDistinct => "COUNT(DISTINCT ?)",
            AggFn::Sum => "SUM",
            AggFn::Avg => "AVG",
            AggFn::Min => "MIN",
            AggFn::Max => "MAX",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            AggFn::Count => "count",
            AggFn::CountDistinct => "count_distinct",
            AggFn::Sum => "sum",
            AggFn::Avg => "avg",
            AggFn::Min => "min",
            AggFn::Max => "max",
        }
    }
    pub const ALL: [AggFn; 6] = [
        AggFn::Count,
        AggFn::CountDistinct,
        AggFn::Sum,
        AggFn::Avg,
        AggFn::Min,
        AggFn::Max,
    ];
}

impl Pipeline {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.steps.len() {
            self.steps.remove(idx);
        }
    }

    pub fn move_up(&mut self, idx: usize) {
        if idx > 0 && idx < self.steps.len() {
            self.steps.swap(idx - 1, idx);
        }
    }

    pub fn move_down(&mut self, idx: usize) {
        if idx + 1 < self.steps.len() {
            self.steps.swap(idx, idx + 1);
        }
    }

    /// Build the full SQL representing the pipeline output.
    pub fn to_sql(&self) -> String {
        let mut current = format!("SELECT * FROM {TABLE_NAME}");
        for step in &self.steps {
            current = step.wrap(&current);
        }
        current
    }

    /// SQL covering only the first `n` steps (n=0 means the base table).
    pub fn prefix_sql(&self, n: usize) -> String {
        let mut current = format!("SELECT * FROM {TABLE_NAME}");
        for step in self.steps.iter().take(n) {
            current = step.wrap(&current);
        }
        current
    }

    /// Build a human-readable CTE chain. Each step becomes its own CTE that selects
    /// from the previous one, so the reader can map step number → SQL fragment.
    pub fn to_pretty_sql(&self) -> String {
        if self.steps.is_empty() {
            return format!("SELECT * FROM {TABLE_NAME}");
        }
        let mut ctes: Vec<String> = Vec::with_capacity(self.steps.len());
        for (i, step) in self.steps.iter().enumerate() {
            let input_sql = if i == 0 {
                format!("SELECT * FROM {TABLE_NAME}")
            } else {
                format!("SELECT * FROM s{}", i)
            };
            let cur_sql = step.wrap(&input_sql);
            ctes.push(format!(
                "  s{n} AS (\n    {cur_sql}\n  )",
                n = i + 1,
                cur_sql = cur_sql,
            ));
        }
        format!(
            "WITH\n{ctes}\nSELECT * FROM s{last}",
            ctes = ctes.join(",\n"),
            last = self.steps.len()
        )
    }
}

impl Step {
    pub fn description(&self) -> String {
        match self {
            Step::Sort {
                column,
                descending,
                nulls_first,
            } => format!(
                "Sort by {column} {} {}",
                if *descending { "DESC" } else { "ASC" },
                if *nulls_first {
                    "NULLS FIRST"
                } else {
                    "NULLS LAST"
                }
            ),
            Step::Filter { predicate } => format!("Filter rows where {predicate}"),
            Step::DropColumns { columns } => format!("Drop columns: {}", columns.join(", ")),
            Step::Rename { from, to } => format!("Rename {from} → {to}"),
            Step::Cast {
                column,
                target_type,
            } => format!("Cast {column} → {target_type}"),
            Step::FillNa { column, value } => format!("Fill nulls in {column} with {value}"),
            Step::NullIf { column, value } => format!("Nullify {column} when equal to {value}"),
            Step::DropNa { columns } => {
                if columns.is_empty() {
                    "Drop rows with any null".into()
                } else {
                    format!("Drop rows with null in: {}", columns.join(", "))
                }
            }
            Step::FindReplace {
                column,
                pattern,
                replacement,
                regex,
            } => format!(
                "Replace in {column}: {pattern} → {replacement}{}",
                if *regex { " (regex)" } else { "" }
            ),
            Step::Lowercase { column } => format!("Lowercase {column}"),
            Step::Uppercase { column } => format!("Uppercase {column}"),
            Step::Strip { column } => format!("Strip whitespace {column}"),
            Step::TextLength { column, new_column } => format!("{new_column} = length({column})"),
            Step::Round { column, decimals } => format!("Round {column} to {decimals} decimals"),
            Step::Floor { column } => format!("Floor {column}"),
            Step::Ceiling { column } => format!("Ceiling {column}"),
            Step::DropDuplicates { columns } => {
                if columns.is_empty() {
                    "Drop duplicate rows (all columns)".into()
                } else {
                    format!("Drop duplicates by: {}", columns.join(", "))
                }
            }
            Step::GroupByAggregate { keys, aggregations } => format!(
                "Group by [{}] aggregate {}",
                keys.join(", "),
                aggregations
                    .iter()
                    .map(|(c, f, a)| format!("{}({c}) AS {a}", f.label()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Step::FormulaColumn {
                new_column,
                expression,
            } => format!("{new_column} = {expression}"),
        }
    }

    fn wrap(&self, prev: &str) -> String {
        match self {
            Step::Sort {
                column,
                descending,
                nulls_first,
            } => {
                let dir = if *descending { "DESC" } else { "ASC" };
                let nulls = if *nulls_first {
                    "NULLS FIRST"
                } else {
                    "NULLS LAST"
                };
                format!(
                    "SELECT * FROM ({prev}) ORDER BY {id} {dir} {nulls}",
                    id = ident(column)
                )
            }
            Step::Filter { predicate } => format!("SELECT * FROM ({prev}) WHERE {predicate}"),
            Step::DropColumns { columns } => {
                let cols = columns
                    .iter()
                    .map(|c| ident(c))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("SELECT * EXCEPT ({cols}) FROM ({prev})")
            }
            Step::Rename { from, to } => {
                let id_from = ident(from);
                let id_to = ident(to);
                // EXCEPT removes the source col from *, then we re-add it under the new name.
                // (Column ends up at the end of the projection — acceptable trade-off.)
                format!("SELECT * EXCEPT ({id_from}), {id_from} AS {id_to} FROM ({prev})")
            }
            Step::Cast {
                column,
                target_type,
            } => {
                let id = ident(column);
                format!("SELECT * REPLACE (CAST({id} AS {target_type}) AS {id}) FROM ({prev})")
            }
            Step::FillNa { column, value } => {
                let id = ident(column);
                format!("SELECT * REPLACE (COALESCE({id}, {value}) AS {id}) FROM ({prev})")
            }
            Step::NullIf { column, value } => {
                let id = ident(column);
                format!("SELECT * REPLACE (NULLIF({id}, {value}) AS {id}) FROM ({prev})")
            }
            Step::DropNa { columns } => {
                if columns.is_empty() {
                    format!("SELECT * FROM ({prev}) WHERE TRUE")
                } else {
                    let conds = columns
                        .iter()
                        .map(|c| format!("{} IS NOT NULL", ident(c)))
                        .collect::<Vec<_>>()
                        .join(" AND ");
                    format!("SELECT * FROM ({prev}) WHERE {conds}")
                }
            }
            Step::FindReplace {
                column,
                pattern,
                replacement,
                regex,
            } => {
                let id = ident(column);
                let pat = sql_string(pattern);
                let rep = sql_string(replacement);
                if *regex {
                    format!(
                        "SELECT * REPLACE (regexp_replace(CAST({id} AS VARCHAR), {pat}, {rep}) AS {id}) FROM ({prev})"
                    )
                } else {
                    format!(
                        "SELECT * REPLACE (replace(CAST({id} AS VARCHAR), {pat}, {rep}) AS {id}) FROM ({prev})"
                    )
                }
            }
            Step::Lowercase { column } => {
                let id = ident(column);
                format!("SELECT * REPLACE (lower(CAST({id} AS VARCHAR)) AS {id}) FROM ({prev})")
            }
            Step::Uppercase { column } => {
                let id = ident(column);
                format!("SELECT * REPLACE (upper(CAST({id} AS VARCHAR)) AS {id}) FROM ({prev})")
            }
            Step::Strip { column } => {
                let id = ident(column);
                format!("SELECT * REPLACE (btrim(CAST({id} AS VARCHAR)) AS {id}) FROM ({prev})")
            }
            Step::TextLength { column, new_column } => {
                let id = ident(column);
                let new = ident(new_column);
                format!("SELECT *, char_length(CAST({id} AS VARCHAR)) AS {new} FROM ({prev})")
            }
            Step::Round { column, decimals } => {
                let id = ident(column);
                format!(
                    "SELECT * REPLACE (round(CAST({id} AS DOUBLE), {decimals}) AS {id}) FROM ({prev})"
                )
            }
            Step::Floor { column } => {
                let id = ident(column);
                format!("SELECT * REPLACE (floor(CAST({id} AS DOUBLE)) AS {id}) FROM ({prev})")
            }
            Step::Ceiling { column } => {
                let id = ident(column);
                format!("SELECT * REPLACE (ceil(CAST({id} AS DOUBLE)) AS {id}) FROM ({prev})")
            }
            Step::DropDuplicates { columns } => {
                if columns.is_empty() {
                    format!("SELECT DISTINCT * FROM ({prev})")
                } else {
                    let cols = columns
                        .iter()
                        .map(|c| ident(c))
                        .collect::<Vec<_>>()
                        .join(", ");
                    // Use ROW_NUMBER window to keep the first row per key group.
                    format!(
                        "SELECT * EXCEPT (__rn) FROM ( \
                            SELECT *, ROW_NUMBER() OVER (PARTITION BY {cols} ORDER BY {cols}) AS __rn \
                            FROM ({prev}) \
                         ) WHERE __rn = 1"
                    )
                }
            }
            Step::GroupByAggregate { keys, aggregations } => {
                let key_ids = keys.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
                let agg_exprs = aggregations
                    .iter()
                    .map(|(col, f, alias)| {
                        let col_id = ident(col);
                        let alias_id = ident(alias);
                        match f {
                            AggFn::CountDistinct => {
                                format!("COUNT(DISTINCT {col_id}) AS {alias_id}")
                            }
                            other => format!("{}({col_id}) AS {alias_id}", other.sql()),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                if keys.is_empty() {
                    format!("SELECT {agg_exprs} FROM ({prev})")
                } else {
                    format!("SELECT {key_ids}, {agg_exprs} FROM ({prev}) GROUP BY {key_ids}")
                }
            }
            Step::FormulaColumn {
                new_column,
                expression,
            } => {
                let new = ident(new_column);
                format!("SELECT *, ({expression}) AS {new} FROM ({prev})")
            }
        }
    }
}

fn ident(name: &str) -> String {
    let escaped = name.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn sql_string(s: &str) -> String {
    let escaped = s.replace('\'', "''");
    format!("'{escaped}'")
}
