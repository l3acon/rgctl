package com.example.ecommerce

class OrderService {
    fun process(order: OrderDTO): OrderDTO {
        order.markProcessed()
        return order
    }

    fun build(status: String): OrderDTO {
        return OrderDTO(status)
    }
}
