package com.example.ecommerce

class OrderService {
    OrderDTO process(OrderDTO order) {
        order.markProcessed()
        return order
    }

    OrderDTO build(String status) {
        return new OrderDTO(status)
    }
}
