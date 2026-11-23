package dev.stroid.app.editor

enum class Token { KEYWORD, STRING, COMMENT, NUMBER, FUNCTION, TYPE }

/** A highlighted range of one line, in UTF-16 indices. Plain text has no span. */
class Span(val start: Int, val end: Int, val token: Token)

/**
 * Languages known to the lightweight highlighter. This is a per-line
 * tokenizer (no multi-line state), a stand-in until tree-sitter is added.
 */
enum class Language(
    val lineComment: String?,
    val keywords: Set<String>,
    val blockComments: Boolean = false,
    val singleQuoteStrings: Boolean = true,
    val capitalizedTypes: Boolean = true,
    val enabled: Boolean = true,
) {
    RUST(
        "//",
        words(
            "as break const continue crate else enum extern false fn for if impl in let loop match mod " +
                "move mut pub ref return self Self static struct super trait true type unsafe use where " +
                "while async await dyn",
        ),
        blockComments = true,
        singleQuoteStrings = false,
    ),
    KOTLIN(
        "//",
        words(
            "as break class continue do else false for fun if in interface is null object override " +
                "package private protected public return super this throw true try typealias val var " +
                "when while import data sealed enum open abstract companion init internal suspend " +
                "lateinit by const inline",
        ),
        blockComments = true,
    ),
    JAVA(
        "//",
        words(
            "abstract boolean break byte case catch char class continue default do double else enum " +
                "extends final finally float for if implements import instanceof int interface long new " +
                "null package private protected public return short static super switch this throw " +
                "throws true false try void volatile while var",
        ),
        blockComments = true,
    ),
    PYTHON(
        "#",
        words(
            "False None True and as assert async await break class continue def del elif else except " +
                "finally for from global if import in is lambda nonlocal not or pass raise return try " +
                "while with yield self",
        ),
    ),
    JAVASCRIPT(
        "//",
        words(
            "break case catch class const continue debugger default delete do else export extends " +
                "false finally for function if import in instanceof let new null return super switch " +
                "this throw true try typeof var void while with yield async await of undefined " +
                "interface type enum implements",
        ),
        blockComments = true,
    ),
    C(
        "//",
        words(
            "auto break case char const continue default do double else enum extern float for goto if " +
                "inline int long register return short signed sizeof static struct switch typedef union " +
                "unsigned void volatile while class namespace new delete template typename public " +
                "private protected virtual using nullptr true false bool",
        ),
        blockComments = true,
    ),
    GO(
        "//",
        words(
            "break case chan const continue default defer else fallthrough for func go goto if import " +
                "interface map package range return select struct switch type var nil true false",
        ),
        blockComments = true,
    ),
    SHELL(
        "#",
        words(
            "if then else elif fi for while do done case esac function in return export local echo " +
                "cd exit set unset",
        ),
        capitalizedTypes = false,
    ),
    CONFIG("#", words("true false"), capitalizedTypes = false),
    JSON(null, words("true false null"), capitalizedTypes = false, singleQuoteStrings = false),
    MARKUP(null, emptySet(), blockComments = false, capitalizedTypes = false),
    PLAIN(null, emptySet(), enabled = false),
    ;

    companion object {
        fun forPath(path: String?): Language {
            val name = path?.substringAfterLast('/')?.lowercase() ?: return PLAIN
            val ext = name.substringAfterLast('.', "")
            return when (ext) {
                "rs" -> RUST
                "kt", "kts" -> KOTLIN
                "java" -> JAVA
                "py" -> PYTHON
                "js", "mjs", "ts", "tsx", "jsx" -> JAVASCRIPT
                "c", "h", "cc", "cpp", "hpp", "cxx" -> C
                "go" -> GO
                "sh", "bash", "zsh" -> SHELL
                "toml", "yml", "yaml", "ini", "properties", "conf", "gitignore" -> CONFIG
                "json" -> JSON
                "html", "htm", "xml", "css", "svg" -> MARKUP
                else -> if (name == "makefile" || name == "dockerfile") CONFIG else PLAIN
            }
        }
    }
}

private fun words(s: String): Set<String> = s.split(' ').filter { it.isNotEmpty() }.toSet()

object Highlighter {
    private fun isIdentStart(c: Char) = c.isLetter() || c == '_'
    private fun isIdentPart(c: Char) = c.isLetterOrDigit() || c == '_'

    fun highlight(line: String, lang: Language): List<Span> {
        if (!lang.enabled || line.isEmpty()) return emptyList()
        val out = ArrayList<Span>()
        val n = line.length
        val comment = lang.lineComment
        var i = 0
        while (i < n) {
            val c = line[i]
            if (comment != null && line.startsWith(comment, i)) {
                out.add(Span(i, n, Token.COMMENT))
                break
            }
            if (lang.blockComments && line.startsWith("/*", i)) {
                val close = line.indexOf("*/", i + 2)
                val end = if (close < 0) n else close + 2
                out.add(Span(i, end, Token.COMMENT))
                i = end
                continue
            }
            if (c == '"' || c == '`' || (c == '\'' && lang.singleQuoteStrings)) {
                var j = i + 1
                while (j < n) {
                    if (line[j] == '\\') {
                        j += 2
                        continue
                    }
                    if (line[j] == c) {
                        j++
                        break
                    }
                    j++
                }
                val end = minOf(j, n)
                out.add(Span(i, end, Token.STRING))
                i = end
                continue
            }
            if (c.isDigit() && (i == 0 || !isIdentPart(line[i - 1]))) {
                var j = i + 1
                while (j < n && (line[j].isLetterOrDigit() || line[j] == '_' || line[j] == '.')) {
                    if (line[j] == '.' && j + 1 < n && line[j + 1] == '.') break
                    j++
                }
                out.add(Span(i, j, Token.NUMBER))
                i = j
                continue
            }
            if (isIdentStart(c)) {
                var j = i + 1
                while (j < n && isIdentPart(line[j])) j++
                val word = line.substring(i, j)
                var k = j
                while (k < n && line[k] == ' ') k++
                val next = if (k < n) line[k] else ' '
                val token = when {
                    word in lang.keywords -> Token.KEYWORD
                    next == '(' || (next == '!' && k + 1 < n && line[k + 1] == '(') -> Token.FUNCTION
                    lang.capitalizedTypes && word.length > 1 && word[0].isUpperCase() &&
                        word.any { it.isLowerCase() } -> Token.TYPE
                    else -> null
                }
                if (token != null) out.add(Span(i, j, token))
                i = j
                continue
            }
            i++
        }
        return out
    }
}
