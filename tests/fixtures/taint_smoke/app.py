def handle(request):
    q = request.GET["id"]
    cursor.execute(q)
