package com.example.edges

// Class with val and var properties
class User {
    val name: String = ""
    var age: Int = 0
    private val id: Long = 0L
}

// Data class with constructor params (val/var)
data class Point(val x: Int, val y: Int)

// Class with methods
class Service {
    fun start() { }
    fun stop() { }
    private fun restart() { }
}

// Interface with methods
interface Repository {
    fun findAll(): List<String>
    fun save(item: String)
}

// Object with methods
object Singleton {
    fun getInstance(): Singleton = this
    fun reset() { }
}

// Top-level function
fun topLevelFunction(): String = "hello"

// Top-level class
class TopLevelClass

// Top-level val
val TOP_CONSTANT: Int = 42

// Top-level var
var topVariable: String = ""
