package demo

interface Repository {
    fun find(id: Long): String
}

open class BaseService

class OrderService(val name: String) : BaseService(), Repository {
    fun validate(x: Int): Int {
        return if (x > 0) x else -x
    }

    override fun find(id: Long): String {
        validate(1)
        return when (id) {
            0L -> "none"
            else -> "order-$id"
        }
    }

    fun tainted(input: String): String {
        // pattern sink for taint fixture
        return input
    }
}
