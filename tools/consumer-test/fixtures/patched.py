class Fixture:
    @property
# 0003 comment at column zero between a decorator and its def
    def value(self) -> int:
        return 1

    @staticmethod
        # a comment indented past the block still works, as before
    def other():
        pass
