async def consume(stream):
    async for item in stream:
        use(item)
    else:
        exhausted()


def iterate(items):
    for item in items:
        use(item)
    else:
        exhausted()

    while ready():
        work()
    else:
        stopped()


def handle():
    try:
        work()
    except* ValueError:
        recover()
    else:
        completed()

    try:
        work()
    except ValueError:
        recover()
    else:
        completed()


match subject:
    case (((Point())) as point):
        use(point)
