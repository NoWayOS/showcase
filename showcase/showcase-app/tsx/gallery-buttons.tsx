// The Screen Compiler gallery, 4 of 11: the three variants, and a button's declared style. Each emits an
// action the application handles; here, none does anything.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Column, HeaderBar, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
  onTry: () => void;
};

export function GalleryButtonsScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Buttons
          </Text>
          <Spacer />
          <Text>
            4 of 11
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
            <Button variant="primary" onPress={props.onTry}>
              Primary
            </Button>
            <Text role="caption">{'variant="primary"'}</Text>
          </Column>
          <Column gap={4}>
            <Button variant="secondary" onPress={props.onTry}>
              Secondary
            </Button>
            <Text role="caption">{'variant="secondary"'}</Text>
          </Column>
          <Column gap={4}>
            <Button variant="quiet" onPress={props.onTry}>
              Quiet
            </Button>
            <Text role="caption">{'variant="quiet"'}</Text>
          </Column>
        </Row>
        <Button onClick={() => props.onTry()} style={{ backgroundColor: "text", color: "background" }}>
          Styled
        </Button>
        <Text role="caption">{'<Button style={{ backgroundColor: "text", color: "background" }}>'}</Text>
        <Text role="caption">{'<Button onPress={props.onSave}>: a callback prop becomes an action'}</Text>
        <Text role="caption">{'<Button onClick={() => props.onSave()}>: React DOM\'s onClick, and a function that only calls a prop'}</Text>
      </Column>
    </Screen>
  );
}
