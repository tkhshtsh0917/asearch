/// Corpus-level engine contract. Queries and workspaces are prepared outside
/// the search hot path; implementations may choose different representations.
pub trait Engine {
    type Query;
    type Workspace;
    type Error;

    /// Returns one minimum level per text, in input order. No match within K
    /// is `None`. Document ranking and key deduplication belong to the index.
    fn search<'w>(
        &self,
        query: &Self::Query,
        workspace: &'w mut Self::Workspace,
    ) -> Result<&'w [Option<u16>], Self::Error>;
}
