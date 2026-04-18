package com.example.tests

import org.junit.Test
import org.junit.Before
import org.junit.After
import org.junit.BeforeClass
import org.junit.AfterClass

// Regular function - NOT a test
fun regularFunction(): String = "hello"

// Annotation-based test detection
class MyTestClass {
    @Before
    fun setUp() {
        // setup
    }

    @After
    fun tearDown() {
        // teardown
    }

    @Test
    fun shouldProcessData() {
        // test body
    }

    @Test
    fun shouldHandleErrors() {
        // test body
    }

    // Regular method - NOT a test
    fun helperMethod(): Int = 42
}

// Companion with lifecycle annotations
class AnotherTestClass {
    companion object {
        @BeforeClass
        fun setupClass() {
            // class setup
        }

        @AfterClass
        fun teardownClass() {
            // class teardown
        }
    }

    @Test
    fun verifyBehavior() {
        // test body
    }
}

// Naming convention tests (no annotations)
fun testUserCreation() {
    // detected by "test" prefix
}

fun testCalculation() {
    // detected by "test" prefix
}

fun userCreationTest() {
    // detected by "Test" suffix
}

fun calculateTest() {
    // detected by "Test" suffix
}

// Should NOT be detected as tests
fun testing() {
    // "testing" != "test" prefix
}

fun contest() {
    // "contest" contains "test" but shouldn't match
}

fun myTestHelper() {
    // "test" is not at beginning, "Test" not at end — but starts with lowercase
}
