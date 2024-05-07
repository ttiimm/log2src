import static java.lang.StringTemplate.STR;

import java.io.IOException;
import java.util.logging.*;

Logger logger = Logger.getLogger("basic");

void main() throws IOException {
    System.setProperty("java.util.logging.SimpleFormatter.format",
                   "%1$tF %1$tT %4$s %2$s: %5$s%6$s%n");
    var fh = new FileHandler("java-basic.log");
    fh.setFormatter(new SimpleFormatter());
    logger.addHandler(fh);
    logger.setLevel(Level.FINE);
    logger.fine("Hello from main");
    for (int i = 0; i < 3; i++) {
        foo(i);
    }
}

void foo(int i) {
    logger.fine(STR."Hello from foo i=\{i}");
}
