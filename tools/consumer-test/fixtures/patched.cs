class Fixture {
    void M(object async) {          // 0002 `async` is contextual, not reserved
        Run(async);
        Run(async: true);
        var resultCode = async;
    }
}
