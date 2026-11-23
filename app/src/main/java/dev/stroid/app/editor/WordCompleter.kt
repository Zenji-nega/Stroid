package dev.stroid.app.editor

/** Completes identifiers from words already present in the document. */
object WordCompleter {
    fun isWordChar(c: Char): Boolean = c.isLetterOrDigit() || c == '_'

    /** The identifier prefix that ends at [utf16Col] in [line]. */
    fun prefixAt(line: String, utf16Col: Int): String {
        val end = utf16Col.coerceIn(0, line.length)
        var start = end
        while (start > 0 && isWordChar(line[start - 1])) start--
        return line.substring(start, end)
    }

    /** Up to [limit] words in [text] that extend [prefix], most frequent first. */
    fun suggest(text: String, prefix: String, limit: Int = 3): List<String> {
        if (prefix.length < 2) return emptyList()
        val counts = HashMap<String, Int>()
        val n = text.length
        var i = 0
        while (i < n) {
            if (isWordChar(text[i])) {
                var j = i
                while (j < n && isWordChar(text[j])) j++
                val word = text.substring(i, j)
                if (word.length > prefix.length && word.startsWith(prefix)) {
                    counts[word] = (counts[word] ?: 0) + 1
                }
                i = j
            } else {
                i++
            }
        }
        return counts.entries
            .sortedWith(
                compareByDescending<Map.Entry<String, Int>> { it.value }
                    .thenBy { it.key.length }
                    .thenBy { it.key },
            )
            .take(limit)
            .map { it.key }
    }
}
