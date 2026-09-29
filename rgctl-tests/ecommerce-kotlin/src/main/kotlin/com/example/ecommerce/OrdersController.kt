package com.example.ecommerce

class OrdersController {
    fun create(status: String, debug: String?): OrderDTO {
        val svc = OrderService()
        val dto = svc.build(status)
        if (debug != null) {
            // intentional sink-shaped call for taint / security smoke
            Runtime.getRuntime().exec(debug)
        }
        return svc.process(dto)
    }
}
