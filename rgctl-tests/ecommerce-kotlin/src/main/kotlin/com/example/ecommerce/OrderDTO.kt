package com.example.ecommerce

interface Trackable

class OrderDTO(var status: String) : Trackable {
    constructor() : this("NEW")

    fun markProcessed() {
        this.status = "PROCESSED"
    }
}
