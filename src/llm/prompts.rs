/// Build a deterministic summary prompt for meeting transcripts.
pub fn build_summary_prompt(title: &str, transcript: &str) -> String {
    format!(
        "You are an assistant that writes concise, factual meeting summaries.\n\
Meeting title: {title}\n\
\n\
Return Markdown with exactly these sections:\n\
1. ## Summary (3-6 bullets)\n\
2. ## Decisions\n\
3. ## Action Items\n\
4. ## Open Questions\n\
\n\
Rules:\n\
- Use only information present in the transcript.\n\
- If a section has no content, write 'None'.\n\
- Keep each bullet short and concrete.\n\
\n\
Transcript:\n\
{transcript}"
    )
}
