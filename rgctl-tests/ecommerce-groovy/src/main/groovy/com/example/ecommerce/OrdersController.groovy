package com.example.ecommerce

class OrdersController {
    OrderDTO create(String status, String debug) {
        def svc = new OrderService()
        def dto = svc.build(status)
        if (debug != null) {
            // intentional sink-shaped call for taint / security smoke
            "sh".execute([debug], null)
        }
        return svc.process(dto)
    }
}
