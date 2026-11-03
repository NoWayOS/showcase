// The Screen Compiler gallery, 8 of 11: the on-screen keyboard, in the layout of the field it types into,
// and a field's error line, which the application sets: Check asks
// for four digits. What is typed here is wiped as the screen closes.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Column, FieldError, HeaderBar, Keyboard, Row, Screen, Spacer, Text, TextInput } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
  onCheck: () => void;
};

export function GalleryKeyboardScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Keyboard and errors
          </Text>
          <Spacer />
          <Text>
            8 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={GAP}>
          <TextInput id="code" placeholder="Four digits" inputMode="numeric" maxLength={4} />
          <Button id="check" variant="secondary" onPress={props.onCheck}>
            Check
          </Button>
        </Row>
        <FieldError for="code" />
        <Text role="caption">{'<FieldError for="code" /> and <Keyboard />'}</Text>
        <Keyboard />
      </Column>
    </Screen>
  );
}
