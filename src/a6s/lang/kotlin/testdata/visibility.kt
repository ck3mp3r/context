package com.example.visibility

// Default visibility (public)
class DefaultClass {
    fun defaultMethod(): String = "hello"
    val defaultProperty: Int = 0
}

// Explicit public
public class PublicClass {
    public fun publicMethod(): String = "hello"
    public val publicProperty: Int = 0
}

// Private
private class PrivateClass {
    private fun privateMethod(): String = "hello"
    private val privateProperty: Int = 0
}

// Protected members (protected only valid on class members)
open class ProtectedExample {
    protected fun protectedMethod(): String = "hello"
    protected val protectedProperty: Int = 0
}

// Internal
internal class InternalClass {
    internal fun internalMethod(): String = "hello"
    internal val internalProperty: Int = 0
}

// Mixed visibility
class MixedVisibility {
    public fun pubFun(): Unit {}
    private fun privFun(): Unit {}
    protected fun protFun(): Unit {}
    internal fun intFun(): Unit {}
    fun defaultFun(): Unit {}
}

// Top-level declarations with visibility
public fun publicTopFun(): Int = 1
private fun privateTopFun(): Int = 2
internal fun internalTopFun(): Int = 3
fun defaultTopFun(): Int = 4

public val publicTopVal: Int = 1
private val privateTopVal: Int = 2
internal val internalTopVal: Int = 3
val defaultTopVal: Int = 4
