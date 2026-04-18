package com.example.functions

class Helper {
    fun assist(): String = "help"
}

fun greet(name: String): String {
    return "Hello, $name"
}

fun caller() {
    greet("world")
    val h = Helper()
    h.assist()
    println("done")
}

fun another() {
    caller()
}
