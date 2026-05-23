package com.example.complex

// Nested classes
class Outer {
    class Nested {
        fun nestedMethod(): String = "nested"
    }

    inner class Inner {
        fun innerMethod(): String = "inner"
    }

    fun outerMethod() {}
}

// Extension functions
fun String.wordCount(): Int = this.split(" ").size

fun Outer.extensionOnOuter(): String = "extended"

// Sealed class hierarchy
sealed class Shape {
    data class Circle(val radius: Double) : Shape()
    data class Rectangle(val width: Double, val height: Double) : Shape()
    object Unknown : Shape()
}

// Companion object
class Factory {
    companion object {
        fun create(): Factory = Factory()
        val DEFAULT_NAME: String = "default"
    }

    fun instanceMethod() {}
}

// Companion object with custom name
class Registry {
    companion object Loader {
        fun load(): Registry = Registry()
    }
}

// Private class (should NOT resolve cross-file)
private class InternalHelper {
    fun helperMethod() {}
}

// Internal class (should resolve within same package only)
internal class PackageHelper {
    fun assist() {}
}

// Public class (should resolve everywhere)
class PublicApi {
    fun serve() {}
}
