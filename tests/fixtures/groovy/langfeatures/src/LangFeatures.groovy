package demo

class OrderService {
  def validate() {}
  def findAll() {
    validate()
    if (true) {
      return "ok"
    }
    return "no"
  }
}
