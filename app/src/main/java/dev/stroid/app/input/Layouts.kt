package dev.stroid.app.input

enum class Lang(val code: String) {
    EN("EN"),
    AR("AR"),
    ;

    fun next(): Lang = if (this == EN) AR else EN
}

enum class Layer { LETTERS, SYMBOLS1, SYMBOLS2 }

/** One key of the on-screen keyboard. [weight] is its relative width. */
sealed class Key(val weight: Float) {
    class Glyph(val text: String, weight: Float = 1f) : Key(weight)
    class Shift(weight: Float = 1.5f) : Key(weight)
    class Backspace(weight: Float = 1.5f) : Key(weight)
    class Enter(weight: Float = 1.6f) : Key(weight)
    class Space(weight: Float = 5f) : Key(weight)
    class LayerSwitch(val label: String, val target: Layer, weight: Float = 1.4f) : Key(weight)
    class LangSwitch(weight: Float = 1.1f) : Key(weight)
}

/**
 * Keyboard layouts. In left-handed mode the keys that are used most while
 * typing with one thumb (backspace, enter) move to the left edge.
 */
object Layouts {
    private fun glyphs(s: String): List<Key> = s.map { Key.Glyph(it.toString()) }
    private fun glyphs(items: List<String>): List<Key> = items.map { Key.Glyph(it) }

    private fun third(edge: Key?, middle: List<Key>, leftHanded: Boolean): List<Key> =
        if (leftHanded) {
            listOf<Key>(Key.Backspace()) + middle + listOfNotNull(edge)
        } else {
            listOfNotNull(edge) + middle + listOf<Key>(Key.Backspace())
        }

    /** Returns four rows: three of keys plus the bottom row. */
    fun rows(lang: Lang, layer: Layer, upper: Boolean, leftHanded: Boolean): List<List<Key>> {
        val row1: List<Key>
        val row2: List<Key>
        val row3: List<Key>
        when (layer) {
            Layer.LETTERS ->
                if (lang == Lang.EN) {
                    fun cased(s: String) = if (upper) s.uppercase() else s
                    row1 = glyphs(cased("qwertyuiop"))
                    row2 = glyphs(cased("asdfghjkl"))
                    row3 = third(Key.Shift(), glyphs(cased("zxcvbnm")), leftHanded)
                } else {
                    row1 = glyphs("ضصثقفغعهخحج")
                    row2 = glyphs("شسيبلاتنمكط")
                    row3 = third(null, glyphs(listOf("ئ", "ء", "ؤ", "ر", "لا", "ى", "ة", "و", "ز", "ظ", "د", "ذ")), leftHanded)
                }
            Layer.SYMBOLS1 -> {
                row1 = glyphs("1234567890")
                row2 = glyphs("@#$%&-+=/\\")
                row3 = third(Key.LayerSwitch("=\\<", Layer.SYMBOLS2), glyphs("!?:;'\".,"), leftHanded)
            }
            Layer.SYMBOLS2 -> {
                row1 = glyphs("~`|^{}[]<>")
                row2 = glyphs("()_*;:'\"!?")
                row3 = third(Key.LayerSwitch("?123", Layer.SYMBOLS1), glyphs("%#@$&-+="), leftHanded)
            }
        }
        val layerKey: Key =
            if (layer == Layer.LETTERS) {
                Key.LayerSwitch("?123", Layer.SYMBOLS1)
            } else {
                Key.LayerSwitch("ABC", Layer.LETTERS)
            }
        val bottom = listOf(layerKey, Key.LangSwitch(), Key.Space(), Key.Enter())
        return listOf(row1, row2, row3, if (leftHanded) bottom.reversed() else bottom)
    }
}
