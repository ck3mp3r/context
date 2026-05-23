package com.example.typerefs

// Custom types for type reference testing
class UserProfile
class Config
class AppError
class Response
class Handler
class Event

// Functions with parameter types
fun process(user: UserProfile, config: Config): Unit { }

// Functions with return types
fun loadConfig(): Config {
    return Config()
}

fun getError(): AppError {
    return AppError()
}

// Properties with explicit types
val activeUser: UserProfile = UserProfile()
var currentConfig: Config = Config()

// Class with typed members
class Service {
    val handler: Handler = Handler()
    var config: Config = Config()

    fun handle(event: Event): Response {
        return Response()
    }
}

// Generic types - should extract base type AND type arguments
fun getUsers(): List<UserProfile> {
    return emptyList()
}

fun getMapping(): Map<String, Config> {
    return emptyMap()
}

fun processItems(items: List<Handler>): Unit { }

// Nullable types - should unwrap to base type
fun findUser(id: Int): UserProfile? {
    return null
}

var optionalConfig: Config? = null

// Lambda/function types
val transformer: (Event) -> Response = { Response() }
val processor: (UserProfile, Config) -> Handler = { _, _ -> Handler() }
