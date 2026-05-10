pub fn minify_graphql(query: &str) -> String {
  query
      .lines()
      .map(|line| line.trim())
      .filter(|line| !line.starts_with('#') && !line.is_empty())
      .collect::<Vec<_>>()
      .join(" ")
}