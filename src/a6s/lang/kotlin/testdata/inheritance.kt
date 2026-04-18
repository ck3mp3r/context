package com.example.inheritance

// Base class (open for extension)
open class Animal(val name: String)

// Simple extends
class Dog(name: String) : Animal(name)

// Interface
interface Runnable {
    fun run()
}

// Another interface
interface Loggable {
    fun log(message: String)
}

// Implements single interface
class Task : Runnable {
    override fun run() { }
}

// Implements multiple interfaces
class Worker : Runnable, Loggable {
    override fun run() { }
    override fun log(message: String) { }
}

// Extends class and implements interface
class ServiceDog(name: String) : Animal(name), Runnable {
    override fun run() { }
}

// Interface extending interface
interface AdvancedRunnable : Runnable {
    fun pause()
}
