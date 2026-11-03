// The Screen Compiler gallery, 6 of 11: a text input in each input mode, with a placeholder and a
// length limit. The application reads and sets each field by its id;
// what is typed here is wiped as the screen closes.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Column, HeaderBar, Row, Screen, Spacer, Text, TextInput } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GalleryFieldsScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Text fields
          </Text>
          <Spacer />
          <Text>
            6 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={EDGE}>
          <Column gap={4}>
            <Text role="caption">{'<TextInput id="name" placeholder="Your name" maxLength={40} />'}</Text>
            <TextInput id="name" placeholder="Your name" inputMode="text" maxLength={40} />
          </Column>
          <Column gap={4}>
            <Text role="caption">{'inputMode="email"'}</Text>
            <TextInput id="email" placeholder="name@example.com" inputMode="email" maxLength={64} />
          </Column>
        </Row>
        <Row gap={EDGE}>
          <Column gap={4}>
            <Text role="caption">{'inputMode="tel": digits only'}</Text>
            <TextInput id="phone" placeholder="Phone" inputMode="tel" maxLength={15} />
          </Column>
          <Column gap={4}>
            <Text role="caption">{'inputMode="numeric": digits only'}</Text>
            <TextInput id="count" placeholder="How many" inputMode="numeric" maxLength={3} />
          </Column>
        </Row>
      </Column>
    </Screen>
  );
}
