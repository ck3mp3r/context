// Fixture for constrained query tests (c5t note 7d1244e0, Task 2).
// Every "should NOT" case is a regression guard for the bare-query bugs.

package com.example.constraints

// === Top-level declarations (SHOULD be extracted) ===
class TopClass {
    fun memberFun(): String = "x"
    val memberVal: Int = 0
    var memberVar: String = ""
}

object TopObject {
    fun objectFun(): Unit {}
    const val CONST_IN_OBJECT: Int = 1
}

interface TopInterface {
    fun ifaceMethod(x: String): Boolean
}

class WithCompanion {
    companion object {
        fun companionFun(): Unit {}
        const val COMPANION_CONST: Int = 2
    }
}

enum class TopEnum {
    RED,
    GREEN,
    BLUE
}

typealias TopTypeAlias = Map<String, Int>

fun topLevelFun(): Unit {}
fun String.topLevelExtensionFun(): Int = this.length

val topLevelVal: String = "top"
var topLevelVar: Int = 0
const val TOP_LEVEL_CONST: Int = 42

// === Local functions and properties (should NOT be extracted) ===
// The Kotlin equivalent of the TS local-variable bug. Bare queries matched
// these because function_declaration / property_declaration appear inside
// function bodies too.
fun withLocals() {
    fun localFun(): Unit {}
    val localVal = 1
    var localVar = 2
    fun String.localExtension(): Int = 0
}

// === Nested classes (SHOULD be extracted — existing test contract) ===
class Outer {
    class Nested {
        fun nestedMethod(): String = "nested"
    }
    inner class Inner {
        fun innerMethod(): String = "inner"
    }
}
