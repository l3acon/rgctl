package com.example.ecommerce

trait Trackable {}

class OrderDTO implements Trackable {
    String status

    OrderDTO(String status) {
        this.status = status
    }

    void markProcessed() {
        this.status = "PROCESSED"
    }
}
