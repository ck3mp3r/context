package com.example.app

// Regular class
class MyClass {
    val name: String = ""
    var count: Int = 0

    fun doSomething(): String {
        return name
    }
}

// Data class
data class Point(val x: Int, val y: Int)

// Sealed class
sealed class Result {
    data class Success(val data: String) : Result()
    data class Error(val message: String) : Result()
}

// Abstract class
abstract class Base {
    abstract fun process(): Boolean
}

// Interface
interface Repository {
    fun findById(id: String): MyClass?
    fun save(item: MyClass)
}

// Functional interface
fun interface Predicate {
    fun test(value: String): Boolean
}

// Object declaration (singleton)
object AppConfig {
    const val MAX_SIZE: Int = 100
    fun getDefault(): String = "default"
}

// Companion object
class Factory {
    companion object {
        fun create(): Factory = Factory()
    }
}

// Enum class
enum class Color {
    RED,
    GREEN,
    BLUE
}

// Type alias
typealias StringMap = Map<String, Int>

// Extension function
fun String.wordCount(): Int {
    return this.split(" ").size
}

// Suspend function
suspend fun fetchData(): String {
    return "data"
}

// Operator function
operator fun Point.plus(other: Point): Point {
    return Point(x + other.x, y + other.y)
}

// Top-level property (val)
val DEFAULT_NAME: String = "unknown"

// Top-level property (var)
var globalCounter: Int = 0

// Inner class example
class Outer {
    inner class Inner {
        fun innerMethod(): String = "inner"
    }
}
